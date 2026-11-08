//! Exact public factor-engine records for scalar and series generic bump paths.

#[path = "../../../valuations/tests/performance/context_bump_fixture.rs"]
mod fixture;

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, PriceId};
use finstack_quant_factor::{
    BumpSizeConfig, FactorDefinition, FactorId, FactorType, MarketMapping,
};
use finstack_quant_portfolio::sensitivity::{DeltaBasedEngine, FactorSensitivityEngine};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::equity::variance_swap::VarianceSwap;
use serde_json::{Value, json};
use time::Duration;

fn bits(value: f64) -> String {
    assert!(value.is_finite());
    format!("{:016x}", value.to_bits())
}

fn factor(target: &str) -> FactorDefinition {
    FactorDefinition {
        id: FactorId::new("CCM-EQUITY-PCT"),
        factor_type: FactorType::Equity,
        market_mapping: MarketMapping::EquitySpot {
            tickers: vec![target.into()],
        },
        description: None,
    }
}

fn engine() -> DeltaBasedEngine {
    DeltaBasedEngine::new(BumpSizeConfig {
        rates_bp: 1.0,
        credit_bp: 1.0,
        equity_pct: 1.0,
        fx_pct: 1.0,
        vol_points: 1.0,
        overrides: Default::default(),
    })
}

fn equity_record(unrelated: usize) -> Value {
    let market = fixture::market(unrelated);
    let instrument = fixture::option();
    let positions = [("CCM-POS".to_string(), &instrument as &dyn Instrument, 1.0)];
    let matrix = engine()
        .compute_sensitivities(
            &positions,
            &[factor("SPX-SPOT")],
            &market,
            fixture::as_of(),
            Currency::USD,
        )
        .expect("generic scalar consumer");
    assert_eq!(matrix.n_positions(), 1);
    assert_eq!(matrix.n_factors(), 1);
    let delta = matrix.delta(0, 0).expect("one delta");
    let reference =
        (fixture::reference_price(101.0, 0.20) - fixture::reference_price(99.0, 0.20)) / 2.0;
    assert!(delta.is_finite());
    assert!((delta - reference).abs() <= 1e-9 * reference.abs().max(1.0));
    json!({
        "case_id": format!("context_bump_consumer/equity_delta_engine/curves_{unrelated}"),
        "role": "generic_money_scalar_consumer", "unrelated_curve_count": unrelated, "total_curve_count": unrelated+1,
        "factor_type": "equity", "factor_units": "percent", "factor_bump": bits(1.0),
        "resolved_multiplier_up": bits(1.01), "resolved_multiplier_down": bits(0.99),
        "target_id": "SPX-SPOT", "position_ids": matrix.position_ids(),
        "factor_ids": matrix.factor_ids().iter().map(|id| id.as_str()).collect::<Vec<_>>(),
        "delta_bits": [bits(delta)], "reference_delta_bits": bits(reference), "currency": "USD", "position_weight": bits(1.0),
        "reference_bound": "1e-9*max(1,abs(reference))", "candidate_comparison_bound": "exact full bits and axes",
    })
}

fn series_record() -> Value {
    let mut market = fixture::market(8);
    let mut swap = VarianceSwap::example().expect("variance swap example");
    swap.id = finstack_quant_core::types::InstrumentId::new("CCM-SERIES-CONTROL");
    swap.spot_id = PriceId::new("SPX-SPOT");
    swap.close_series_id = Some("SPX-CLOSE".into());
    swap.vol_surface_id = CurveId::new("CCM-EQ-VOL");
    swap.div_yield_id = Some(PriceId::new("CCM-DIV"));
    swap.discount_curve_id = CurveId::new("CCM-USD-OIS");
    swap.start_date = fixture::as_of() - Duration::days(31);
    swap.maturity = fixture::as_of() + Duration::days(30);
    swap.notional = Money::new(1000.0, Currency::USD).expect("variance notional");
    // Keep future variance independent of the realized closes in this route control.
    swap.instrument_pricing_overrides
        .market_quotes
        .implied_volatility = Some(0.20);
    let observations: Vec<_> = (0..=31)
        .map(|index| {
            (
                swap.start_date + Duration::days(index),
                100.0 * (0.001 * index as f64 + 0.002 * (index as f64).sin()).exp(),
            )
        })
        .collect();
    market.insert_series_mut(
        ScalarTimeSeries::new("SPX-CLOSE", observations.clone(), Some(Currency::USD))
            .expect("realized closes"),
    );
    assert!(market.get_price("SPX-CLOSE").is_err());
    assert!(market.curve("SPX-CLOSE").is_none());
    assert!(market.get_surface("SPX-CLOSE").is_err());
    let dependencies = swap.market_dependencies().expect("variance dependencies");
    assert!(dependencies.series_ids.iter().any(|id| id == "SPX-CLOSE"));
    let used_closes = swap
        .get_historical_prices(&market, fixture::as_of())
        .expect("exact dated history");
    assert!(used_closes.len() >= 2);
    let positions = [("CCM-SERIES-POS".to_string(), &swap as &dyn Instrument, 1.0)];
    let matrix = engine()
        .compute_sensitivities(
            &positions,
            &[factor("SPX-CLOSE")],
            &market,
            fixture::as_of(),
            Currency::USD,
        )
        .expect("series-only factor consumer");
    let delta = matrix.delta(0, 0).expect("one series sensitivity");
    let base = swap
        .value_raw(&market, fixture::as_of())
        .expect("variance base price");
    // A near-zero result alone would not establish that a historical-series
    // repricing is meaningful. Pin declared series dependencies and independently
    // replay both public endpoint markets, every used close and their prices.
    let mut endpoint_prices = Vec::new();
    let mut raw_endpoint_prices = Vec::new();
    let mut endpoints = Vec::new();
    for multiplier in [1.01, 0.99] {
        let endpoint = market
            .bump([MarketBump::Curve {
                id: CurveId::new("SPX-CLOSE"),
                spec: BumpSpec::multiplier(multiplier),
            }])
            .expect("series-only endpoint");
        let closes = swap
            .get_historical_prices(&endpoint, fixture::as_of())
            .expect("endpoint dated closes");
        assert_eq!(closes.len(), used_closes.len());
        for (&actual, &original) in closes.iter().zip(&used_closes) {
            assert_eq!(actual.to_bits(), (original * multiplier).to_bits());
        }
        let raw_price = swap
            .value_raw(&endpoint, fixture::as_of())
            .expect("endpoint variance price");
        let (currency_price, currency) = swap
            .value_raw_with_currency(&endpoint, fixture::as_of())
            .expect("currency-aware endpoint variance price");
        assert_eq!(currency, Currency::USD);
        assert_eq!(raw_price.to_bits(), currency_price.to_bits());
        // Match the public currency representation used by raw_pv_in_base,
        // including its Money view even for same-currency positions. Retain
        // the raw endpoint separately so conversion roundoff is observable.
        let portfolio_money = Money::new(currency_price, currency).expect("finite endpoint Money");
        let portfolio_price = portfolio_money.amount();
        endpoint_prices.push(portfolio_price);
        raw_endpoint_prices.push(raw_price);
        endpoints.push(json!({
            "multiplier_bits": bits(multiplier),
            "used_close_bits": closes.into_iter().map(bits).collect::<Vec<_>>(),
            "raw_pv_bits": bits(raw_price),
            "currency_aware_raw_pv_bits": bits(currency_price),
            "portfolio_currency_view_decimal_mantissa": portfolio_money.amount_decimal().mantissa().to_string(),
            "portfolio_currency_view_decimal_scale": portfolio_money.amount_decimal().scale(),
            "portfolio_currency_view_pv_bits": bits(portfolio_price),
            "currency": "USD",
        }));
    }
    let explicit_delta = (endpoint_prices[0] - endpoint_prices[1]) / 2.0;
    let raw_delta = (raw_endpoint_prices[0] - raw_endpoint_prices[1]) / 2.0;
    assert_eq!(
        delta.to_bits(),
        explicit_delta.to_bits(),
        "factor engine must match public currency-aware endpoint reprices"
    );
    assert!(delta.is_finite());
    assert!(raw_delta.is_finite());
    assert!(
        raw_delta.abs() <= 1e-9 * base.abs().max(1.0),
        "raw uniform-close-scaling reprices must preserve return variance to roundoff"
    );
    assert!(
        delta.abs() <= 1e-9 * base.abs().max(1.0),
        "uniform close scaling must preserve return variance to roundoff"
    );
    json!({
        "case_id": "context_bump_consumer/variance_series_invariance/curves_8", "role": "series_only_consumer_invariance_control_not_spot_delta",
        "target_id": "SPX-CLOSE", "spot_id": "SPX-SPOT", "factor_bump_percent": bits(1.0),
        "full_series": observations.iter().map(|(date,value)| json!({"date":date.to_string(),"value_bits":bits(*value)})).collect::<Vec<_>>(),
        "used_close_count": used_closes.len(), "used_close_bits": used_closes.into_iter().map(bits).collect::<Vec<_>>(),
        "declared_series_ids": dependencies.series_ids, "explicit_endpoint_records": endpoints,
        "explicit_endpoint_delta_bits": bits(explicit_delta),
        "raw_endpoint_delta_bits": bits(raw_delta),
        "endpoint_comparison_boundary": "value_raw_with_currency, then Money::new and amount; same-currency FX identity, matching the portfolio workflow",
        "base_pv_bits": bits(base), "delta_bits": bits(delta), "currency": "USD",
        "reference_bound": "abs(delta)<=1e-9*max(1,abs(base_pv))", "candidate_comparison_bound": "exact all numeric and structural records",
    })
}

#[test]
fn context_bump_factor_consumers_match_scalar_and_series_contracts() {
    let _ = equity_record(8);
    let _ = series_record();
}

#[test]
#[ignore = "exports frozen CCM I1 factor-consumer references on explicit request"]
fn export_context_bump_consumer_reference() {
    let directory =
        std::env::var_os("CCM_CONSUMER_EXPORT_DIR").expect("CCM_CONSUMER_EXPORT_DIR is required");
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).expect("create export directory");
    let report = json!({"schema_version":1, "panel":"ccm_i1_factor_consumers", "records":[equity_record(1024),series_record()]});
    if let Some(reference) = std::env::var_os("CCM_FACTOR_REFERENCE_FILE") {
        let expected: Value =
            serde_json::from_slice(&std::fs::read(reference).expect("read reference"))
                .expect("parse reference");
        assert_eq!(
            report, expected,
            "I1 factor outputs/configuration must match C0 exactly"
        );
    }
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("factor-consumer-reference.json"))
        .expect("create new panel without overwriting evidence");
    std::io::Write::write_all(
        &mut output,
        &serde_json::to_vec_pretty(&report).expect("serialize panel"),
    )
    .expect("write panel");
}
