//! Regression coverage for multi-asset factor model configuration.

use finstack_core::currency::Currency;
use finstack_core::market_data::bumps::BumpUnits;
use finstack_core::types::CurveId;
use finstack_factor_model::{
    AttributeFilter, BumpSizeConfig, DependencyFilter, DependencyType, FactorCovarianceMatrix,
    FactorDefinition, FactorId, FactorModelConfig, FactorType, MappingRule, MarketMapping,
    MatchingConfig, PricingMode, RiskMeasure, UnmatchedPolicy,
};

#[test]
fn factor_model_config_supports_multi_asset_factor_universe() {
    let factors = vec![
        curve_factor(
            "rates::usd_ois",
            FactorType::Rates,
            "USD-OIS",
            BumpUnits::RateBp,
        ),
        curve_factor(
            "credit::acme_hzd",
            FactorType::Credit,
            "ACME-HZD",
            BumpUnits::RateBp,
        ),
        FactorDefinition {
            id: FactorId::new("equity::aapl"),
            factor_type: FactorType::Equity,
            market_mapping: MarketMapping::EquitySpot {
                tickers: vec!["AAPL".to_string()],
            },
            description: Some("AAPL spot factor".to_string()),
        },
        FactorDefinition {
            id: FactorId::new("fx::eur_usd"),
            factor_type: FactorType::FX,
            market_mapping: MarketMapping::FxRate {
                pair: (Currency::EUR, Currency::USD),
            },
            description: None,
        },
        FactorDefinition {
            id: FactorId::new("vol::spx"),
            factor_type: FactorType::Volatility,
            market_mapping: MarketMapping::VolShift {
                surface_ids: vec!["SPX-VOL".to_string()],
                units: BumpUnits::RateBp,
            },
            description: None,
        },
        curve_factor(
            "commodity::wti",
            FactorType::Commodity,
            "WTI-FWD",
            BumpUnits::Fraction,
        ),
        curve_factor(
            "inflation::usd_cpi",
            FactorType::Inflation,
            "USD-CPI",
            BumpUnits::RateBp,
        ),
    ];
    let factor_ids: Vec<FactorId> = factors.iter().map(|factor| factor.id.clone()).collect();
    let covariance = FactorCovarianceMatrix::new(factor_ids.clone(), identity(factor_ids.len()))
        .expect("identity covariance is valid");

    let matching = MatchingConfig::MappingTable(
        factor_ids
            .iter()
            .map(|factor_id| MappingRule {
                dependency_filter: DependencyFilter::default(),
                attribute_filter: AttributeFilter {
                    tags: vec![factor_id.as_str().to_string()],
                    meta: vec![],
                },
                factor_id: factor_id.clone(),
            })
            .collect(),
    );

    let config = FactorModelConfig {
        factors,
        covariance,
        matching,
        pricing_mode: PricingMode::DeltaBased,
        risk_measure: RiskMeasure::Variance,
        bump_size: Some(BumpSizeConfig::default()),
        unmatched_policy: Some(UnmatchedPolicy::Strict),
    };

    config
        .validate_matching_factor_ids()
        .expect("all mapping-table factors are declared");
    let round_trip: FactorModelConfig =
        serde_json::from_str(&serde_json::to_string(&config).expect("serialize config"))
            .expect("deserialize config");
    assert_eq!(round_trip.factors.len(), 7);
    assert_eq!(
        round_trip.covariance.factor_ids(),
        config.covariance.factor_ids()
    );
}

#[test]
fn matching_config_validation_rejects_undeclared_non_credit_factor() {
    let factors = vec![curve_factor(
        "rates::usd_ois",
        FactorType::Rates,
        "USD-OIS",
        BumpUnits::RateBp,
    )];
    let covariance = FactorCovarianceMatrix::new(vec![FactorId::new("rates::usd_ois")], vec![1.0])
        .expect("single factor covariance is valid");
    let config = FactorModelConfig {
        factors,
        covariance,
        matching: MatchingConfig::MappingTable(vec![MappingRule {
            dependency_filter: DependencyFilter {
                dependency_type: Some(DependencyType::Spot),
                curve_type: None,
                id: None,
            },
            attribute_filter: AttributeFilter::default(),
            factor_id: FactorId::new("equity::missing"),
        }]),
        pricing_mode: PricingMode::DeltaBased,
        risk_measure: RiskMeasure::Variance,
        bump_size: None,
        unmatched_policy: None,
    };

    assert!(config.validate_matching_factor_ids().is_err());
}

fn curve_factor(
    id: &str,
    factor_type: FactorType,
    curve_id: &str,
    units: BumpUnits,
) -> FactorDefinition {
    FactorDefinition {
        id: FactorId::new(id),
        factor_type,
        market_mapping: MarketMapping::CurveParallel {
            curve_ids: vec![CurveId::new(curve_id)],
            units,
        },
        description: None,
    }
}

fn identity(n: usize) -> Vec<f64> {
    let mut data = vec![0.0; n * n];
    for idx in 0..n {
        data[idx * n + idx] = 1.0;
    }
    data
}
