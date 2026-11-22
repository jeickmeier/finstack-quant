use super::replication::{cumulative_variation, replicated_variance_to};
use super::*;
use crate::fx_variance_swap::FxVarianceSwap;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::dates::DayCountContext;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::{VolQuoteType, VolSurfaceAxis};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
use finstack_quant_models::closed_form::bs_price_unchecked;
use finstack_quant_models::volatility::variance_replication::carr_madan_forward_variance;
use finstack_quant_valuations_core::instruments::Instrument;
use std::sync::Arc;
use time::macros::date;

fn build_market(as_of: Date) -> MarketContext {
    let usd_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (1.0, (-0.03_f64).exp())])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("usd curve");
    let eur_curve = DiscountCurve::builder("EUR-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (1.0, (-0.01_f64).exp())])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("eur curve");
    let provider = SimpleFxProvider::new();
    provider
        .set_quotes(&[(Currency::EUR, Currency::USD, 1.10)])
        .expect("valid rate");
    let fx = FxMatrix::new(Arc::new(provider), None);
    MarketContext::new()
        .try_insert(usd_curve)
        .expect("insert market curve")
        .try_insert(eur_curve)
        .expect("insert market curve")
        .insert_fx(fx)
}

fn zero_rate_market(as_of: Date, volatility: f64) -> MarketContext {
    use finstack_quant_core::market_data::surfaces::{VolInterpolationMode, VolSurface};
    let mut market = build_market(as_of);
    for id in ["USD-OIS", "EUR-OIS"] {
        market = market
            .try_insert(
                DiscountCurve::builder(id)
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (5.0, 1.0)])
                    .day_count(finstack_quant_core::dates::DayCount::Act365F)
                    .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                    .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                    .build()
                    .expect("curve"),
            )
            .expect("insert market curve");
    }
    let provider = SimpleFxProvider::new();
    provider
        .set_quotes(&[(Currency::EUR, Currency::USD, 1.0)])
        .expect("spot");
    let strikes: Vec<_> = (1..=400).map(|i| f64::from(i) * 0.01).collect();
    let vols = vec![volatility; strikes.len()];
    market
        .insert_fx(FxMatrix::new(Arc::new(provider), None))
        .insert_surface(
            VolSurface::builder("EURUSD-VOL")
                .expiries(&[1.0, 3.0])
                .secondary_values(&strikes)
                .secondary_axis(VolSurfaceAxis::Strike)
                .quote_type(VolQuoteType::BlackLognormal)
                .interpolation_mode(VolInterpolationMode::Vol)
                .row(&vols)
                .row(&vols)
                .build()
                .expect("surface"),
        )
}

#[test]
fn replication_rejects_non_black_and_non_strike_surfaces() {
    use finstack_quant_core::market_data::surfaces::VolSurface;
    let swap = FxVarianceSwap::example().expect("swap");
    let market = zero_rate_market(swap.start_date, 0.2);
    let base = market
        .get_surface(swap.vol_surface_id.as_str())
        .expect("surface");
    for surface in [
        VolSurface::from_grid(
            base.id(),
            base.expiries(),
            base.secondary_values(),
            base.vols(),
            finstack_quant_core::market_data::surfaces::VolGridOpts {
                secondary_axis: base.secondary_axis(),
                quote_type: VolQuoteType::ShiftedBlackLognormal,
                interpolation_mode: base.interpolation_mode(),
            },
            Some(&[0.1; 2]),
        )
        .expect("shifted surface"),
        finstack_quant_valuations_test_support::volatility::relabel_quote_type(
            base.as_ref(),
            VolQuoteType::Normal,
        )
        .unwrap(),
        VolSurface::from_grid(
            base.id(),
            base.expiries(),
            base.secondary_values(),
            base.vols(),
            finstack_quant_core::market_data::surfaces::VolGridOpts {
                secondary_axis: VolSurfaceAxis::Tenor,
                quote_type: base.quote_type(),
                interpolation_mode: base.interpolation_mode(),
            },
            None,
        )
        .expect("tenor surface"),
    ] {
        let context = market.clone().insert_surface(surface);
        let error = replicated_variance_to(&swap, &context, swap.start_date, swap.maturity)
            .expect_err("Carr-Madan Black replication must enforce surface metadata");
        let text = error.to_string();
        assert!(
            text.contains("quotes") || text.contains("secondary axis"),
            "{text}"
        );
    }
}

#[test]
fn fixed_returns_keep_the_same_denominator_before_final_fixing() {
    use finstack_quant_core::dates::Tenor;
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    for frequency in [Tenor::daily(), Tenor::weekly(), Tenor::monthly()] {
        for annualization in [252.0, 365.0] {
            let mut swap = FxVarianceSwap::example().expect("example");
            swap.start_date = date!(2025 - 01 - 08);
            swap.maturity = if frequency == Tenor::daily() {
                date!(2025 - 01 - 13)
            } else {
                date!(2025 - 05 - 08)
            };
            swap.observation_frequency = frequency;
            swap.trading_days_per_year = annualization;
            swap.strike_variance = 0.0;
            let dates = observation_dates(&swap).expect("dates");
            let final_date = *dates.last().expect("last");
            let as_of = final_date - time::Duration::days(1);
            let market = zero_rate_market(as_of, 0.00001);
            // Exactly two known returns, +1% then -1%; all remaining
            // historical closes and the final close are 1.0.
            let observations: Vec<_> = dates
                .iter()
                .enumerate()
                .map(|(i, &date)| (date, if i == 1 { 0.01_f64.exp() } else { 1.0 }))
                .collect();
            let control_observations = dates.iter().map(|&date| (date, 1.0)).collect();
            let known = market.clone().insert_series(
                ScalarTimeSeries::new(
                    "EURUSD",
                    observations,
                    None,
                    finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
                    finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
                )
                .expect("series"),
            );
            let control = market.insert_series(
                ScalarTimeSeries::new(
                    "EURUSD",
                    control_observations,
                    None,
                    finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
                    finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
                )
                .expect("series"),
            );
            let before_difference = expected_variance(&swap, &known, as_of).expect("known")
                - expected_variance(&swap, &control, as_of).expect("control");
            let exact = annualization_factor(&swap) * 0.0002 / (dates.len() - 1) as f64;
            assert!((before_difference - exact).abs() < 1e-12);
            let settled = expected_variance(&swap, &known, final_date).expect("settled");
            assert!((settled - exact).abs() < 1e-12);
        }
    }
}

#[test]
fn forward_start_uses_only_the_observed_window_and_discrete_mean_correction() {
    use finstack_quant_core::dates::Tenor;
    use finstack_quant_core::market_data::surfaces::{VolInterpolationMode, VolSurface};
    let as_of = date!(2025 - 01 - 06);
    let mut swap = FxVarianceSwap::example().expect("example");
    swap.start_date = date!(2026 - 01 - 06);
    swap.maturity = date!(2027 - 01 - 06);
    swap.observation_frequency = Tenor::monthly();
    let dates = observation_dates(&swap).expect("dates");
    let times: Vec<_> = dates
        .iter()
        .map(|&date| {
            swap.day_count
                .year_fraction(as_of, date, DayCountContext::default())
                .expect("time")
        })
        .collect();
    // Deterministic instantaneous variance: 1% until year 1, then 17%.
    // Both intervals together imply 30% annual vol at year 2.
    let integrated = |time: f64| 0.01 * time.min(1.0) + 0.17 * (time - 1.0).max(0.0);
    let strikes: Vec<_> = (1..=800).map(|i| f64::from(i) * 0.005).collect();
    let mut surface = VolSurface::builder("EURUSD-VOL")
        .expiries(&times)
        .secondary_values(&strikes)
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol);
    for &time in &times {
        surface = surface.row(&vec![(integrated(time) / time).sqrt(); strikes.len()]);
    }
    let market = zero_rate_market(as_of, 0.1).insert_surface(surface.build().expect("surface"));
    let expected = times
        .windows(2)
        .map(|window| {
            let q = integrated(window[1]) - integrated(window[0]);
            q + 0.25 * q * q
        })
        .sum::<f64>()
        * annualization_factor(&swap)
        / (dates.len() - 1) as f64;
    let actual = expected_variance(&swap, &market, as_of).expect("forward variance");
    assert!(
        (actual - expected).abs() < 0.00005,
        "{actual} vs {expected}"
    );
    let mut earlier = swap;
    earlier.start_date = as_of;
    let earlier_value = expected_variance(&earlier, &market, as_of).expect("earlier start");
    assert!(actual > earlier_value + 0.04);
}

#[test]
fn fx_variance_vega_reprices_the_same_sample_normalized_model() {
    use crate::fx_variance_swap::metrics::VegaCalculator;
    use finstack_quant_core::dates::Tenor;
    use finstack_quant_valuations_core::metrics::bump_surface_vol_absolute;
    use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};
    let as_of = date!(2025 - 01 - 06);
    let mut swap = FxVarianceSwap::example().expect("example");
    swap.start_date = date!(2026 - 01 - 06);
    swap.maturity = date!(2027 - 01 - 06);
    swap.observation_frequency = Tenor::monthly();
    let market = zero_rate_market(as_of, 0.2);
    let up = bump_surface_vol_absolute(&market, "EURUSD-VOL", 0.01).expect("up");
    let down = bump_surface_vol_absolute(&market, "EURUSD-VOL", -0.01).expect("down");
    let expected = (swap.value(&up, as_of).expect("up PV").amount()
        - swap.value(&down, as_of).expect("down PV").amount())
        / 2.0;
    let base = swap.value(&market, as_of).expect("base");
    let mut context = MetricContext::new(
        Arc::new(swap),
        Arc::new(market),
        as_of,
        base,
        MetricContext::default_config(),
    );
    let actual = VegaCalculator.calculate(&mut context).expect("vega");
    assert!((actual - expected).abs() < 1e-6);
}

#[test]
fn negative_forward_variation_is_rejected() {
    use finstack_quant_core::dates::Tenor;
    use finstack_quant_core::market_data::surfaces::{VolInterpolationMode, VolSurface};
    let as_of = date!(2025 - 01 - 06);
    let mut swap = FxVarianceSwap::example().expect("example");
    swap.start_date = date!(2026 - 01 - 06);
    swap.maturity = date!(2027 - 01 - 06);
    swap.observation_frequency = Tenor::annual();
    let strikes: Vec<_> = (1..=400).map(|i| f64::from(i) * 0.01).collect();
    let high = vec![0.4; strikes.len()];
    let low = vec![0.1; strikes.len()];
    let surface = VolSurface::builder("EURUSD-VOL")
        .expiries(&[1.0, 2.0])
        .secondary_values(&strikes)
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol)
        .row(&high)
        .row(&low)
        .build()
        .expect("surface");
    let market = zero_rate_market(as_of, 0.1).insert_surface(surface);
    assert!(
        expected_variance(&swap, &market, as_of)
            .expect_err("calendar arbitrage")
            .to_string()
            .contains("negative forward variation")
    );
}

#[test]
fn ohlc_forecast_is_explicitly_unsupported_but_single_bar_realization_works() {
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    use finstack_quant_core::math::stats::RealizedVarMethod;
    let as_of = date!(2025 - 01 - 06);
    let mut swap = FxVarianceSwap::example().expect("example");
    swap.start_date = as_of;
    swap.maturity = date!(2025 - 01 - 08);
    swap.realized_var_method = RealizedVarMethod::Parkinson;
    swap.open_series_id = Some("OPEN".into());
    swap.high_series_id = Some("HIGH".into());
    swap.low_series_id = Some("LOW".into());
    let mut market = zero_rate_market(as_of, 0.2);
    for (name, value) in [
        ("OPEN", 1.0),
        ("HIGH", 1.01),
        ("LOW", 0.99),
        ("EURUSD", 1.0),
    ] {
        market = market.insert_series(
            ScalarTimeSeries::new(
                name,
                vec![(as_of, value)],
                None,
                finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
                finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
            )
            .expect("series"),
        );
    }
    let realized = partial_realized_variance(&swap, &market, as_of).expect("single bar");
    let exact = (1.01_f64 / 0.99).ln().powi(2) * annualization_factor(&swap) / (4.0 * 2.0_f64.ln());
    assert!((realized - exact).abs() < 1e-12);
    assert!(
        expected_variance(&swap, &market, as_of)
            .expect_err("no OHLC model")
            .to_string()
            .contains("estimator-specific")
    );
}

#[test]
fn realized_fraction_uses_each_estimators_sample_count() {
    use finstack_quant_core::math::stats::RealizedVarMethod;
    let mut swap = FxVarianceSwap::example().expect("example");
    swap.start_date = date!(2025 - 01 - 06);
    swap.maturity = date!(2025 - 01 - 13);
    let dates = observation_dates(&swap).expect("six business dates");
    assert_eq!(dates.len(), 6);
    for method in [
        RealizedVarMethod::CloseToClose,
        RealizedVarMethod::YangZhang,
        RealizedVarMethod::Parkinson,
        RealizedVarMethod::GarmanKlass,
        RealizedVarMethod::RogersSatchell,
    ] {
        swap.realized_var_method = method;
        let anchored = method == RealizedVarMethod::CloseToClose;
        for (index, &date) in dates.iter().enumerate() {
            let expected = if anchored {
                index as f64 / 5.0
            } else {
                (index + 1) as f64 / 6.0
            };
            let actual = realized_fraction_by_observations(&swap, date).expect("fraction");
            assert!(
                (actual - expected).abs() < 1e-12,
                "{method:?}: {actual} vs {expected}"
            );
        }
    }
}

#[test]
fn unfinished_return_includes_the_move_since_the_last_fixing() {
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    let as_of = date!(2025 - 01 - 12);
    let mut swap = FxVarianceSwap::example().expect("example");
    swap.start_date = date!(2025 - 01 - 08);
    swap.maturity = date!(2025 - 01 - 13);
    let dates = observation_dates(&swap).expect("dates");
    let observations = dates
        .iter()
        .copied()
        .filter(|&date| date <= as_of)
        .map(|date| (date, 1.0))
        .collect();
    let provider = SimpleFxProvider::new();
    provider
        .set_quotes(&[(Currency::EUR, Currency::USD, 1.02)])
        .expect("spot");
    let market = zero_rate_market(as_of, 0.2)
        .insert_fx(FxMatrix::new(Arc::new(provider), None))
        .insert_series(
            ScalarTimeSeries::new(
                "EURUSD",
                observations,
                None,
                finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
                finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
            )
            .expect("series"),
        );
    let q = cumulative_variation(&swap, &market, as_of, swap.maturity).expect("future variation");
    let mean = 1.02_f64.ln() - 0.5 * q;
    let expected = annualization_factor(&swap) * (q + mean * mean) / 3.0;
    let actual = expected_variance(&swap, &market, as_of).expect("variance");
    assert!((actual - expected).abs() < 1e-12);
    let without_partial_move = annualization_factor(&swap) * (q + 0.25 * q * q) / 3.0;
    assert!((actual - without_partial_move).abs() > 0.01);
}

#[test]
fn fully_observed_ohlc_settlement_preserves_estimator_definitions() {
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    use finstack_quant_core::math::stats::RealizedVarMethod;
    let mut swap = FxVarianceSwap::example().expect("example");
    swap.start_date = date!(2025 - 01 - 06);
    swap.maturity = date!(2025 - 01 - 09);
    swap.open_series_id = Some("OPEN".into());
    swap.high_series_id = Some("HIGH".into());
    swap.low_series_id = Some("LOW".into());
    let dates = observation_dates(&swap).expect("four bars");
    assert_eq!(dates.len(), 4);
    let mut market = zero_rate_market(swap.start_date, 0.2);
    for (id, first, later) in [
        ("OPEN", 1.0, 1.0),
        ("EURUSD", 1.0, 1.0),
        ("HIGH", 1.2, 1.01),
        ("LOW", 0.8, 0.99),
    ] {
        let mut observations: Vec<_> = dates
            .iter()
            .enumerate()
            .map(|(i, &date)| (date, if i == 0 { first } else { later }))
            .collect();
        if id == "EURUSD" {
            observations.insert(0, (date!(2025 - 01 - 03), 1.0));
        }
        market = market.insert_series(
            ScalarTimeSeries::new(
                id,
                observations,
                None,
                finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
                finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
            )
            .expect("series"),
        );
    }
    let range_mean = ((1.2_f64 / 0.8).ln().powi(2) + 3.0 * (1.01_f64 / 0.99).ln().powi(2)) / 4.0;
    let rs_first = 1.2_f64.ln().powi(2) + 0.8_f64.ln().powi(2);
    let rs_later = 1.01_f64.ln().powi(2) + 0.99_f64.ln().powi(2);
    for (method, per_period) in [
        (
            RealizedVarMethod::Parkinson,
            range_mean / (4.0 * 2.0_f64.ln()),
        ),
        (RealizedVarMethod::GarmanKlass, 0.5 * range_mean),
        (
            RealizedVarMethod::RogersSatchell,
            (rs_first + 3.0 * rs_later) / 4.0,
        ),
        // Four target bars follow the separately observed Friday close.
        (
            RealizedVarMethod::YangZhang,
            (1.0 - 0.34 / (1.34 + 5.0 / 3.0)) * (rs_first + 3.0 * rs_later) / 4.0,
        ),
    ] {
        swap.realized_var_method = method;
        swap.previous_close_date =
            (method == RealizedVarMethod::YangZhang).then_some(date!(2025 - 01 - 03));
        let expected = annualization_factor(&swap) * per_period;
        let actual = expected_variance(&swap, &market, swap.maturity).expect("settlement variance");
        assert!(
            (actual - expected).abs() < 1e-12,
            "{method:?}: {actual} vs {expected}"
        );
        let pv = swap
            .value(&market, swap.maturity)
            .expect("settlement PV")
            .amount();
        assert!((pv - swap.payoff(expected).expect("payoff").amount()).abs() < 1e-7);
    }
}

#[test]
fn fx_variance_swap_pricer_compute_pv_matches_instrument_value() {
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    let swap = FxVarianceSwap::example().expect("example");
    let as_of = date!(2025 - 01 - 02);
    let observations = observation_dates(&swap)
        .expect("observation schedule")
        .into_iter()
        .map(|date| (date, 1.10))
        .collect();
    let series = ScalarTimeSeries::new(
        "EURUSD",
        observations,
        None,
        finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
        finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
    )
    .expect("series");
    let market = build_market(as_of).insert_series(series);

    let via_pricer = compute_pv(&swap, &market, as_of).expect("pricer pv");
    let via_instrument = swap.value(&market, as_of).expect("instrument pv");

    assert_eq!(via_pricer, via_instrument);
}

#[test]
fn multi_day_tenor_steps_in_business_observations() {
    use finstack_quant_core::dates::{Tenor, TenorUnit};

    let mut swap = FxVarianceSwap::example().expect("example");
    swap.start_date = date!(2025 - 01 - 03); // Friday
    swap.maturity = date!(2025 - 01 - 15);
    swap.observation_frequency = Tenor::new(2, TenorUnit::Days).expect("valid tenor fixture");

    let dates = observation_dates(&swap).expect("observation schedule");
    assert_eq!(annualization_factor(&swap), 126.0);
    assert_eq!(dates[0], date!(2025 - 01 - 03));
    assert_eq!(dates[1], date!(2025 - 01 - 07));
    assert!(
        dates
            .iter()
            .all(|d| !matches!(d.weekday(), time::Weekday::Saturday | time::Weekday::Sunday))
    );

    swap.observation_frequency = Tenor::new(2, TenorUnit::Weeks).expect("valid tenor fixture");
    assert_eq!(annualization_factor(&swap), 26.0);
}

/// The full sample denominator is identical before and at settlement;
/// calendar-time weights do not change already fixed return contributions.
#[test]
fn fx_seasoned_mtm_uses_contractual_sample_normalization() {
    use finstack_quant_core::dates::{DayCount, Tenor};
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    use finstack_quant_core::market_data::surfaces::{VolInterpolationMode, VolSurface};
    use finstack_quant_core::math::stats::RealizedVarMethod;
    use finstack_quant_core::types::Attributes;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use finstack_quant_valuations_core::instruments::PayReceive;

    let start = date!(2025 - 01 - 06); // Monday
    let maturity = date!(2025 - 06 - 30); // Monday
    let as_of = date!(2025 - 06 - 27); // Friday, near maturity

    let swap = FxVarianceSwap::builder()
        .id(InstrumentId::new("FXVAR-SEASONED"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .spot_id("EURUSD".into())
        .notional(Money::from((1_000_000_i64, Currency::USD)))
        .strike_variance(0.04)
        .start_date(start)
        .maturity(maturity)
        .observation_frequency(Tenor::daily())
        .base_calendar_id("TARGET2".into())
        .quote_calendar_id("USNY".into())
        .realized_var_method(RealizedVarMethod::CloseToClose)
        .side(PayReceive::Receive)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .vol_surface_id(CurveId::new("EURUSD-VOL"))
        .day_count(DayCount::Act365F)
        .attributes(Attributes::new())
        .build()
        .expect("seasoned fx swap");

    let count_w = realized_fraction_by_observations(&swap, as_of).expect("observation fraction");
    // Calendar-time weight over the observation window: the counterfactual
    // the observation-count weighting replaces.
    let final_observation = *observation_dates(&swap)
        .expect("observation schedule")
        .last()
        .expect("observation dates");
    let time_w = swap
        .day_count
        .year_fraction(swap.start_date, as_of, DayCountContext::default())
        .expect("elapsed")
        / swap
            .day_count
            .year_fraction(
                swap.start_date,
                final_observation,
                DayCountContext::default(),
            )
            .expect("window");
    assert!(
        (count_w - time_w).abs() > 1e-4,
        "schedule must make count weight ({count_w}) differ from time weight ({time_w})"
    );

    // Close series over every past observation date with a non-trivial
    // return path so realized variance != forward variance.
    let past: Vec<Date> = observation_dates(&swap)
        .expect("observation schedule")
        .into_iter()
        .filter(|&d| d <= as_of)
        .collect();
    let obs: Vec<(Date, f64)> = past
        .iter()
        .enumerate()
        .map(|(i, &d)| (d, 1.10 * (1.0 + 0.002 * (i as f64 % 3.0 - 1.0))))
        .collect();
    let series = ScalarTimeSeries::new(
        "EURUSD",
        obs,
        None,
        finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
        finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
    )
    .expect("series");
    let surface = VolSurface::builder("EURUSD-VOL")
        .expiries(&[1.0])
        .secondary_values(&[0.9, 1.1, 1.3])
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol)
        .row(&[0.12, 0.10, 0.12])
        .build()
        .expect("surface");
    let market = build_market(as_of)
        .insert_series(series)
        .insert_surface(surface);

    let pv = compute_pv(&swap, &market, as_of).expect("seasoned fx pv");

    let realized = partial_realized_variance(&swap, &market, as_of).expect("realized");
    let forward = remaining_forward_variance(&swap, &market, as_of).expect("forward");
    let expected_var = realized * count_w + forward * (1.0 - count_w);
    let dom = market.get_discount("USD-OIS").expect("curve");
    // Date-based discounting, matching the pricer (item 4).
    let df = dom
        .df_between_dates(as_of, swap.maturity)
        .expect("date-based df");
    let expected_pv = swap
        .payoff(expected_var)
        .expect("valid payoff")
        .checked_mul_f64(df)
        .expect("finite discount factor");

    assert!(
        (pv.amount() - expected_pv.amount()).abs() < 1e-6,
        "FX seasoned MTM must preserve the sample denominator: pv={} expected={}",
        pv.amount(),
        expected_pv.amount()
    );

    let count_var = realized * time_w + forward * (1.0 - time_w);
    let count_pv = swap
        .payoff(count_var)
        .expect("valid payoff")
        .checked_mul_f64(df)
        .expect("finite discount factor");
    assert!(
        (pv.amount() - count_pv.amount()).abs() > 1e-6,
        "FX seasoned MTM must differ from calendar-time weighting"
    );
}

/// W39 regression: `remaining_forward_variance` must recover the GK forward
/// via date-based discount factors. The buggy code calls `curve.df(yf(as_of,
/// mat))`, which looks up the *spot* DF at `yf(as_of, mat)` years from the
/// curve's `base_date` — i.e. the wrong time point on the curve's axis.
/// The correct DF for the period `[as_of, mat]` is
/// `curve.df_between_dates(as_of, mat)` = `df(yf(base, mat)) / df(yf(base, as_of))`.
///
/// These two differ whenever the curve is non-flat (i.e. the spot rate at
/// `yf(as_of, mat)` from `base` ≠ the forward rate over `[as_of, mat]`).
/// The fixture uses a three-knot stepped curve (r_high for [0, as_of],
/// r_low for [as_of, mat]) so the two lookups diverge materially.
#[test]
fn fx_variance_swap_forward_recovery_is_date_based() {
    use finstack_quant_core::dates::{DayCount, DayCountContext, Tenor};
    use finstack_quant_core::market_data::surfaces::{VolInterpolationMode, VolSurface};
    use finstack_quant_core::math::stats::RealizedVarMethod;
    use finstack_quant_core::types::Attributes;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use finstack_quant_valuations_core::instruments::PayReceive;

    // Three dates:
    //   base_date  = 2025-01-02  (curve anchor)
    //   as_of      = 2025-07-01  (~0.5y from base)
    //   maturity   = 2026-01-02  (~1.0y from base, ~0.5y from as_of)
    //
    // Stepped (non-flat) curves so the spot DF at `yf(as_of, mat)` from base
    // is materially different from the forward DF over `[as_of, mat]`:
    //   Domestic: r_near = 10% for [base, as_of], r_far = 0% for [as_of, mat].
    //     df_between(as_of, mat) ≈ 1.0  (zero rate going forward)
    //     dom.df(yf(as_of,mat) ≈ 0.5) ≈ exp(-0.10*0.5) ≈ 0.951 (WRONG — reads near segment)
    //   Foreign: r_near = 0% for [base, as_of], r_far = 10% for [as_of, mat].
    //     df_between(as_of, mat) ≈ exp(-0.10*0.5) ≈ 0.951
    //     for_curve.df(0.5)      ≈ 1.0             (WRONG — reads near segment)
    //
    //   Date-based fwd  ≈ spot * exp((r_d=0 − r_f=0.10) * 0.5) ≈ 1.10 * 0.951 ≈ 1.046
    //   Axis-buggy  fwd ≈ spot * exp((r_d≈10% − r_f≈0%) * 0.5) ≈ 1.10 * 1.051 ≈ 1.156
    //   Gap ≈ 0.11 >> 1e-3.
    //
    // Vol surface has a strong strike slope so replicated variance is
    // sensitive to the recovered forward.
    let curve_base = date!(2025 - 01 - 02);
    let as_of = date!(2025 - 07 - 01);
    let start = date!(2025 - 07 - 02);
    let maturity = date!(2026 - 01 - 02);

    // Knot times in Act365F from curve_base.
    // t_near ≈ yf(2025-01-02, 2025-07-01) = 180/365 ≈ 0.4932
    // t_mat  ≈ yf(2025-01-02, 2026-01-02) = 365/365 = 1.0000
    let t_near: f64 = 0.4932;
    let t_mat: f64 = 1.0000;

    // Domestic: r_near = 10%, r_far = 0%.
    let df_near_dom = (-0.10_f64 * t_near).exp();
    let df_mat_dom = df_near_dom; // exp(-0 * segment) = 1, so no further discount

    // Foreign: r_near = 0%, r_far = 10%.
    let df_near_for = 1.0_f64; // exp(-0 * t_near) = 1
    let df_mat_for = df_near_for * (-0.10_f64 * (t_mat - t_near)).exp();

    let usd_curve = DiscountCurve::builder("USD-OIS")
        .base_date(curve_base)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (t_near, df_near_dom), (t_mat, df_mat_dom)])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("usd curve");
    let eur_curve = DiscountCurve::builder("EUR-OIS")
        .base_date(curve_base)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (t_near, df_near_for), (t_mat, df_mat_for)])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("eur curve");
    let provider = SimpleFxProvider::new();
    provider
        .set_quotes(&[(Currency::EUR, Currency::USD, 1.10)])
        .expect("valid rate");
    // Dense wings support Carr-Madan replication while preserving a strong slope.
    let surface = VolSurface::builder("EURUSD-VOL")
        .expiries(&[1.0])
        .secondary_values(&[0.80, 0.90, 1.00, 1.10, 1.20, 1.30])
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol)
        .row(&[0.34, 0.30, 0.24, 0.18, 0.10, 0.05])
        .build()
        .expect("surface");
    let market = MarketContext::new()
        .try_insert(usd_curve)
        .expect("insert market curve")
        .try_insert(eur_curve)
        .expect("insert market curve")
        .insert_fx(FxMatrix::new(Arc::new(provider), None))
        .insert_surface(surface);

    let swap = FxVarianceSwap::builder()
        .id(InstrumentId::new("FXVAR-FWD-W39"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .spot_id("EURUSD".into())
        .notional(Money::from((1_000_000_i64, Currency::USD)))
        .strike_variance(0.04)
        .start_date(start)
        .maturity(maturity)
        .observation_frequency(Tenor::daily())
        .base_calendar_id("TARGET2".into())
        .quote_calendar_id("USNY".into())
        .realized_var_method(RealizedVarMethod::CloseToClose)
        .side(PayReceive::Receive)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .vol_surface_id(CurveId::new("EURUSD-VOL"))
        .day_count(DayCount::Act365F)
        .attributes(Attributes::new())
        .build()
        .expect("pre-start fx swap");

    // ── Verify the fixture exposes a meaningful gap ───────────────────────
    let dom = market.get_discount("USD-OIS").expect("usd curve");
    let for_curve = market.get_discount("EUR-OIS").expect("eur curve");

    // Correct: forward DF from as_of to maturity.
    let df_dom_date = dom.df_between_dates(as_of, maturity).expect("date df dom");
    let df_for_date = for_curve
        .df_between_dates(as_of, maturity)
        .expect("date df for");

    // Buggy: spot DF at yf(as_of, mat) from base_date.
    let t_dom_axis = dom
        .day_count()
        .year_fraction(as_of, maturity, DayCountContext::default())
        .expect("yf dom");
    let t_for_axis = for_curve
        .day_count()
        .year_fraction(as_of, maturity, DayCountContext::default())
        .expect("yf for");
    let df_dom_bug = dom.df(t_dom_axis.max(0.0)).expect("df");
    let df_for_bug = for_curve.df(t_for_axis.max(0.0)).expect("df");

    assert!(
        (df_dom_date - df_dom_bug).abs() > 1e-3,
        "fixture must expose domestic DF gap: date={df_dom_date} axis={df_dom_bug}"
    );
    assert!(
        (df_for_date - df_for_bug).abs() > 1e-3,
        "fixture must expose foreign DF gap: date={df_for_date} axis={df_for_bug}"
    );

    // ── Derive expected (date-based) and buggy forwards ───────────────────
    let spot = 1.10_f64;
    let t = swap
        .day_count
        .year_fraction(as_of, maturity, DayCountContext::default())
        .expect("yf");
    let r_d_date =
        finstack_quant_valuations_core::instruments::pricing::discounting::zero_rate_from_df(
            df_dom_date,
            t,
            "dom",
        )
        .expect("r_d date");
    let r_f_date =
        finstack_quant_valuations_core::instruments::pricing::discounting::zero_rate_from_df(
            df_for_date,
            t,
            "for",
        )
        .expect("r_f date");
    let fwd_expected = spot * ((r_d_date - r_f_date) * t).exp();

    let r_d_bug =
        finstack_quant_valuations_core::instruments::pricing::discounting::zero_rate_from_df(
            df_dom_bug, t, "dom",
        )
        .expect("r_d bug");
    let r_f_bug =
        finstack_quant_valuations_core::instruments::pricing::discounting::zero_rate_from_df(
            df_for_bug, t, "for",
        )
        .expect("r_f bug");
    let fwd_bug = spot * ((r_d_bug - r_f_bug) * t).exp();

    assert!(
        (fwd_expected - fwd_bug).abs() > 1e-3,
        "fixture must produce different GK forwards: date={fwd_expected} axis={fwd_bug}"
    );

    // ── Assert the fixed pricer uses the date-based forward ───────────────
    // Recompute Carr-Madan with each candidate forward and assert the pricer
    // follows the date-based one.
    let surface_ref = market.get_surface("EURUSD-VOL").expect("surface");
    let strikes = surface_ref.secondary_values();
    let vol_fn = |t_exp: f64, k: f64| {
        finstack_quant_models::volatility::get_surface_vol_clamped(&surface_ref, t_exp, k)
    };
    let expected_variance =
        carr_madan_forward_variance(strikes, fwd_expected, r_d_date, t, vol_fn, |k, v, opt| {
            bs_price_unchecked(spot, k, r_d_date, r_f_date, v, t, opt)
        })
        .expect("date-based replication");
    let bug_variance =
        carr_madan_forward_variance(strikes, fwd_bug, r_d_bug, t, vol_fn, |k, v, opt| {
            bs_price_unchecked(spot, k, r_d_bug, r_f_bug, v, t, opt)
        })
        .expect("axis-bug replication");

    let actual = replicated_variance_to(&swap, &market, as_of, maturity)
        .expect("forward variance must succeed");

    if (expected_variance - bug_variance).abs() > 1e-8 {
        assert!(
            (actual - expected_variance).abs() < (actual - bug_variance).abs(),
            "remaining_forward_variance must use date-based forward: \
             actual={actual} date_expected={expected_variance} axis_bug={bug_variance}"
        );
    }
}

/// Item 4 regression: the terminal PV discount must be date-based. When the
/// discount curve's `base_date` precedes `as_of`, `dom.df(yf(as_of, mat))`
/// reads the curve at the wrong point on its time axis; the correct factor
/// is `dom.df_between_dates(as_of, maturity)`.
#[test]
fn fx_variance_swap_terminal_discount_is_date_based() {
    use finstack_quant_core::dates::{DayCount, Tenor};
    use finstack_quant_core::market_data::surfaces::{VolInterpolationMode, VolSurface};
    use finstack_quant_core::math::stats::RealizedVarMethod;
    use finstack_quant_core::types::Attributes;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use finstack_quant_valuations_core::instruments::PayReceive;

    // Curve base date is well before the valuation date (a stale-but-valid
    // curve). `df(t)` is anchored at base_date, so feeding it
    // `yf(as_of, maturity)` mis-discounts.
    let curve_base = date!(2025 - 01 - 02);
    let as_of = date!(2025 - 07 - 01);
    let start = date!(2025 - 07 - 02);
    let maturity = date!(2026 - 01 - 02);

    let usd_curve = DiscountCurve::builder("USD-OIS")
        .base_date(curve_base)
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (2.0, (-0.06_f64).exp())])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("usd curve");
    let eur_curve = DiscountCurve::builder("EUR-OIS")
        .base_date(curve_base)
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (2.0, (-0.02_f64).exp())])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("eur curve");
    let provider = SimpleFxProvider::new();
    provider
        .set_quotes(&[(Currency::EUR, Currency::USD, 1.10)])
        .expect("valid rate");
    let surface = VolSurface::builder("EURUSD-VOL")
        .expiries(&[1.0])
        .secondary_values(&[0.9, 1.1, 1.3])
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol)
        .row(&[0.12, 0.10, 0.12])
        .build()
        .expect("surface");
    let market = MarketContext::new()
        .try_insert(usd_curve)
        .expect("insert market curve")
        .try_insert(eur_curve)
        .expect("insert market curve")
        .insert_fx(FxMatrix::new(Arc::new(provider), None))
        .insert_surface(surface);

    let swap = FxVarianceSwap::builder()
        .id(InstrumentId::new("FXVAR-DISC"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .spot_id("EURUSD".into())
        .notional(Money::from((1_000_000_i64, Currency::USD)))
        .strike_variance(0.04)
        .start_date(start)
        .maturity(maturity)
        .observation_frequency(Tenor::daily())
        .base_calendar_id("TARGET2".into())
        .quote_calendar_id("USNY".into())
        .realized_var_method(RealizedVarMethod::CloseToClose)
        .side(PayReceive::Receive)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .vol_surface_id(CurveId::new("EURUSD-VOL"))
        .day_count(DayCount::Act365F)
        .attributes(Attributes::new())
        .build()
        .expect("pre-start fx swap");

    let pv = compute_pv(&swap, &market, as_of).expect("pre-start pv");

    let forward = remaining_forward_variance(&swap, &market, as_of).expect("forward");
    let dom = market.get_discount("USD-OIS").expect("curve");
    let df_correct = dom
        .df_between_dates(as_of, maturity)
        .expect("date-based df");
    let expected_pv = swap
        .payoff(forward)
        .expect("valid payoff")
        .checked_mul_f64(df_correct)
        .expect("finite discount factor");
    assert!(
        (pv.amount() - expected_pv.amount()).abs() < 1e-6,
        "terminal PV must use date-based discounting: pv={} expected={}",
        pv.amount(),
        expected_pv.amount()
    );

    // The buggy time-axis lookup gives a materially different factor.
    let t_bug = swap
        .day_count
        .year_fraction(as_of, maturity, DayCountContext::default())
        .expect("yf");
    let df_bug = dom.df(t_bug.max(0.0)).expect("df");
    assert!(
        (df_correct - df_bug).abs() > 1e-4,
        "fixture must expose the discount-axis bug: df_correct={df_correct} df_bug={df_bug}"
    );
}
