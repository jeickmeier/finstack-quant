//! Independent reciprocal-NDF and total-trade barrier rebate contracts.

use finstack_quant_core::{
    currency::Currency,
    dates::{Date, DayCount},
    market_data::{
        context::MarketContext, scalars::MarketScalar, surfaces::VolSurface,
        term_structures::DiscountCurve,
    },
    money::Money,
    types::BarrierType,
};
use finstack_quant_models::closed_form::barrier::RebateTiming;
use finstack_quant_valuations::instruments::{
    exotics::barrier_option::BarrierOption,
    fx::ndf::{Ndf, NdfQuoteConvention},
    Instrument,
};
use time::macros::date;

fn market(as_of: Date) -> MarketContext {
    MarketContext::new()
        .insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (2.0, 1.0)])
                .build()
                .expect("zero discount curve"),
        )
        .insert_price("SPX-SPOT", MarketScalar::Unitless(100.0))
        .insert_surface(
            VolSurface::builder("SPX-VOL")
                .expiries(&[1.0])
                .strikes(&[100.0])
                .row(&[0.2])
                .build()
                .expect("volatility"),
        )
}

#[test]
fn production_barrier_ndf_reciprocal_quotes_keep_long_base_payoff() {
    let as_of = date!(2024 - 01 - 02);
    let market = market(as_of);
    let mut inverse = Ndf::example();
    inverse.base_currency = Currency::CNY;
    inverse.settlement_currency = Currency::USD;
    inverse.notional = Money::from((7_000_000_i64, Currency::CNY));
    inverse.fixing_date = date!(2024 - 06 - 28);
    inverse.maturity = date!(2024 - 07 - 02);
    inverse.contract_rate = 7.0;
    inverse.fixing_rate = Some(8.0);
    inverse.domestic_discount_curve_id = "USD-OIS".into();
    inverse.quote_convention = NdfQuoteConvention::BasePerSettlement;
    let mut direct = inverse.clone();
    direct.quote_convention = NdfQuoteConvention::SettlementPerBase;
    direct.contract_rate = 1.0 / 7.0;
    direct.fixing_rate = Some(1.0 / 8.0);
    // Long 7m CNY loses USD value when CNY weakens from 7 to 8 per USD:
    // 7m / 8 - 7m / 7 = -125,000 USD in either quotation convention.
    for contract in [&inverse, &direct] {
        let pv = contract.value(&market, as_of).expect("NDF value");
        assert_eq!(pv.currency(), Currency::USD);
        assert!((pv.amount() + 125_000.0).abs() < 1e-8, "{pv}");
    }
    for (contract, expected_sign) in [(inverse, -1.0), (direct, 1.0)] {
        let fixing = contract.fixing_rate.expect("observed fixing");
        let bump = fixing * 1e-4;
        let mut up = contract.clone();
        let mut down = contract;
        up.fixing_rate = Some(fixing + bump);
        down.fixing_rate = Some(fixing - bump);
        let change = up.value(&market, as_of).expect("up fixing").amount()
            - down.value(&market, as_of).expect("down fixing").amount();
        assert!(change * expected_sign > 0.0);
    }
}

fn barrier() -> BarrierOption {
    let mut option = BarrierOption::example().expect("barrier option");
    option.strike = 100.0;
    option.barrier = Money::from((120_i64, Currency::USD));
    option.barrier_type = BarrierType::UpAndOut;
    option.expiry = date!(2025 - 01 - 02);
    option.day_count = DayCount::Act365F;
    option.div_yield_id = None;
    option.monitoring = finstack_quant_valuations::instruments::Monitoring::Continuous;
    option.notional = Money::from((1_000_i64, Currency::USD));
    option.rebate = Some(Money::from((25_i64, Currency::USD)));
    option
}

#[test]
fn production_barrier_ndf_known_expiry_rebate_is_total_trade_money() {
    let as_of = date!(2024 - 01 - 02);
    let market = market(as_of);
    let mut option = barrier();
    option.observed_barrier_breached = Some(true);
    option.rebate_timing = RebateTiming::AtExpiry;
    for scale in [1.0, 1_000.0] {
        option.notional = Money::new(scale, Currency::USD).expect("notional");
        assert!(
            (option.value(&market, as_of).expect("known rebate").amount() - 25.0).abs() < 1e-10
        );
    }
}

#[test]
fn production_barrier_ndf_expired_at_hit_rebate_has_already_been_paid() {
    let mut option = barrier();
    option.observed_barrier_breached = Some(true);
    option.rebate_timing = RebateTiming::AtHit;
    option.expiry_fixing = Some(Money::from((100_i64, Currency::USD)));
    for as_of in [option.expiry, date!(2025 - 01 - 03)] {
        let value = option.value(&market(as_of), as_of).expect("expired value");
        assert_eq!(value.amount(), 0.0);
    }
}

#[test]
fn production_barrier_ndf_quanto_uses_asset_financing_and_payoff_discounting() {
    use finstack_quant_valuations::instruments::exotics::range_accrual::{
        BoundsType, RangeAccrual,
    };
    let as_of = date!(2025 - 01 - 02);
    let expiry = date!(2026 - 01 - 02);
    let market = market(as_of)
        .insert(
            DiscountCurve::builder("EUR-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (2.0, (-0.2_f64).exp())])
                .build()
                .expect("asset financing curve"),
        )
        .insert_price(
            "SPX-SPOT",
            MarketScalar::Price(Money::from((100_i64, Currency::EUR))),
        )
        .insert_price("EURUSD", MarketScalar::Unitless(1.1))
        .insert_surface(
            VolSurface::builder("FX-VOL")
                .expiries(&[1.0])
                .strikes(&[1.0])
                .row(&[0.1])
                .build()
                .expect("FX volatility"),
        );
    let mut note = RangeAccrual::example();
    note.observation_dates = vec![expiry];
    note.accrual_start_date = as_of;
    note.day_count = DayCount::Act365F;
    note.lower_bound = 80.0;
    note.upper_bound = 120.0;
    note.bounds_type = BoundsType::Absolute;
    note.div_yield_id = None;
    note.quanto = Some(
        serde_json::from_value(serde_json::json!({
            "asset_currency": "EUR",
            "asset_discount_curve_id": "EUR-OIS",
            "correlation": 0.5,
            "fx_vol_surface_id": "FX-VOL",
            "fx_spot_id": "EURUSD"
        }))
        .expect("quanto inputs"),
    );
    // EUR asset financing is 10%; correlation is to USD per EUR. Under
    // the USD measure, log(S_T/S_0) has mean .10 - .5*.2*.1 - .5*.2^2.
    // The coupon is paid in USD and its USD discount factor is one.
    let log_mean = 0.1 - 0.5 * 0.2 * 0.1 - 0.5 * 0.2 * 0.2;
    let probability =
        finstack_quant_core::math::special_functions::norm_cdf(((1.2_f64).ln() - log_mean) / 0.2)
            - finstack_quant_core::math::special_functions::norm_cdf(
                ((0.8_f64).ln() - log_mean) / 0.2,
            );
    let expected = 100_000.0 * 0.08 * probability;
    let value = note.value(&market, as_of).expect("quanto range accrual");
    assert_eq!(value.currency(), Currency::USD);
    assert!(
        (value.amount() - expected).abs() < 0.02,
        "{value} versus {expected}"
    );
    note.instrument_pricing_overrides.model_config.mc_paths = Some(20_000);
    use finstack_quant_valuations::pricer::{
        standard_pricer_registry, InstrumentType, ModelKey, PricerKey,
    };
    let simulated = standard_pricer_registry()
        .get_pricer(PricerKey::new(
            InstrumentType::RangeAccrual,
            ModelKey::MonteCarloGBM,
        ))
        .expect("range MC pricer")
        .price_dyn(&note, &market, as_of)
        .expect("quanto range MC")
        .value
        .amount();
    // The one-observation coupon is Bernoulli; 100 USD is over 3.5 standard
    // errors at 20k paths, and is well below the reproduced 378 USD drift bias.
    assert!(
        (simulated - expected).abs() < 100.0,
        "MC {simulated}, independent expectation {expected}"
    );
    // The active asset quote must also change the covariance adjustment and
    // the interval distribution; it is not merely surface-risk metadata.
    note.instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(0.4);
    let log_mean = 0.1 - 0.5 * 0.4 * 0.1 - 0.5 * 0.4 * 0.4;
    let expected_override = 8_000.0
        * (finstack_quant_core::math::special_functions::norm_cdf(
            ((1.2_f64).ln() - log_mean) / 0.4,
        ) - finstack_quant_core::math::special_functions::norm_cdf(
            ((0.8_f64).ln() - log_mean) / 0.4,
        ));
    let overridden = note
        .value(&market, as_of)
        .expect("active quanto volatility")
        .amount();
    assert!(
        (overridden - expected_override).abs() < 0.02,
        "active volatility: {overridden}, independent {expected_override}"
    );
}

fn barrier_model_value(
    option: &BarrierOption,
    market: &MarketContext,
    as_of: Date,
    model: finstack_quant_valuations::pricer::ModelKey,
) -> f64 {
    use finstack_quant_valuations::pricer::{standard_pricer_registry, InstrumentType, PricerKey};
    standard_pricer_registry()
        .get_pricer(PricerKey::new(InstrumentType::BarrierOption, model))
        .expect("registered barrier model")
        .price_dyn(option, market, as_of)
        .expect("barrier model value")
        .value
        .amount()
}

#[test]
fn production_barrier_ndf_all_engines_keep_known_rebate_units_and_lifecycle() {
    use finstack_quant_valuations::pricer::ModelKey;
    let mut option = barrier();
    option.observed_barrier_breached = Some(true);
    option.expiry_fixing = Some(Money::new(100.0, Currency::USD).expect("fixing"));
    for model in [
        ModelKey::BarrierBSContinuous,
        ModelKey::MonteCarloGBM,
        ModelKey::MonteCarloHeston,
        ModelKey::PdeCrankNicolson1D,
    ] {
        for as_of in [date!(2024 - 01 - 02), option.expiry, date!(2025 - 01 - 03)] {
            option.rebate_timing = RebateTiming::AtExpiry;
            assert!(
                (barrier_model_value(&option, &market(as_of), as_of, model) - 25.0).abs() < 1e-10,
                "{model:?}/{as_of}"
            );
            option.rebate_timing = RebateTiming::AtHit;
            assert_eq!(
                barrier_model_value(&option, &market(as_of), as_of, model),
                0.0,
                "{model:?}/{as_of}"
            );
        }
    }
}

#[test]
fn production_barrier_ndf_discrete_gbm_and_heston_do_not_observe_between_dates() {
    use finstack_quant_valuations::{
        instruments::{Monitoring, OptionType},
        pricer::ModelKey,
    };
    let as_of = date!(2024 - 01 - 02);
    let expiry = date!(2025 - 01 - 02);
    let t = DayCount::Act365F
        .year_fraction(as_of, expiry, Default::default())
        .expect("time");
    let market = market(as_of)
        .insert_price("DIV", MarketScalar::Unitless(-(0.8_f64).ln() / t))
        .insert_price("HESTON_KAPPA", MarketScalar::Unitless(1.0))
        .insert_price("HESTON_THETA", MarketScalar::Unitless(1e-16))
        .insert_price("HESTON_V0", MarketScalar::Unitless(1e-16))
        .insert_price("HESTON_SIGMA_V", MarketScalar::Unitless(1e-12))
        .insert_price("HESTON_RHO", MarketScalar::Unitless(0.0))
        .insert_surface(
            VolSurface::builder("SPX-VOL")
                .expiries(&[1.0])
                .strikes(&[100.0])
                .row(&[1e-8])
                .build()
                .expect("near-zero vol"),
        );
    let mut option = barrier();
    option.option_type = OptionType::Put;
    option.notional = Money::new(1.0, Currency::USD).expect("notional");
    option.barrier = Money::new(90.0, Currency::USD).expect("barrier");
    option.rebate = Some(Money::new(2.5, Currency::USD).expect("rebate"));
    option.rebate_timing = RebateTiming::AtHit;
    option.div_yield_id = Some("DIV".into());
    option.instrument_pricing_overrides.model_config.mc_paths = Some(128);
    for model in [ModelKey::MonteCarloGBM, ModelKey::MonteCarloHeston] {
        option.monitoring = Monitoring::Continuous;
        assert!((barrier_model_value(&option, &market, as_of, model) - 2.5).abs() < 1e-10);
        option.monitoring = Monitoring::Discrete {
            observation_dates: vec![expiry],
        };
        // Spot starts above the up barrier but ends at 80, below its only
        // contractual observation. The terminal put pays 100 - 80 = 20.
        let discrete = barrier_model_value(&option, &market, as_of, model);
        assert!((discrete - 20.0).abs() < 0.001, "{model:?}: {discrete}");
    }
}
