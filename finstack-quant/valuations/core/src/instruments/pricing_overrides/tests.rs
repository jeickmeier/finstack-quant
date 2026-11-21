use super::*;

mod price_driver_tests {
    use super::MarketQuoteOverrides;

    /// Set every named field through the wire, so a name that is not a real
    /// field fails to deserialize.
    fn with_fields(fields: &[&str]) -> MarketQuoteOverrides {
        let quotes: serde_json::Map<String, serde_json::Value> = fields
            .iter()
            .map(|field| ((*field).to_string(), serde_json::json!(1.0)))
            .collect();
        serde_json::from_value(serde_json::Value::Object(quotes)).expect("wire field names")
    }

    #[test]
    fn published_fields_are_the_wire_names_the_count_reads() {
        let fields = MarketQuoteOverrides::price_driver_fields();
        assert_eq!(fields.len(), 10);
        assert_eq!(with_fields(&fields).price_driver_count(), fields.len());
        for field in &fields {
            assert_eq!(with_fields(&[field]).price_driver_count(), 1, "{field}");
        }
    }

    #[test]
    fn non_driving_quotes_are_not_in_the_set() {
        let fields = MarketQuoteOverrides::price_driver_fields();
        for field in ["implied_volatility", "quoted_premium", "cds_quote_bp"] {
            assert!(!fields.contains(&field));
            assert_eq!(with_fields(&[field]).price_driver_count(), 0, "{field}");
        }
    }
}

#[test]
fn black_karasinski_model_config_round_trips_and_validates() {
    let config: ModelConfig = serde_json::from_value(serde_json::json!({
        "tree_model": "black_karasinski",
        "bk_sigma": 0.20,
        "bk_mean_reversion": 0.03
    }))
    .expect("Black-Karasinski model_config");
    assert_eq!(config.tree_model, Some(ShortRateTreeModel::BlackKarasinski));
    assert_eq!(config.bk_sigma, Some(0.20));
    assert_eq!(config.bk_mean_reversion, Some(0.03));
    config.validate().expect("valid Black-Karasinski inputs");

    let json = serde_json::to_value(&config).expect("serialize");
    assert_eq!(json["tree_model"], "black_karasinski");
    assert_eq!(json["bk_sigma"], 0.20);
    assert_eq!(json["bk_mean_reversion"], 0.03);
    let back: ModelConfig = serde_json::from_value(json).expect("round trip");
    assert_eq!(back, config);

    let default_json = serde_json::to_value(ModelConfig::default()).expect("serialize");
    assert!(default_json.get("bk_sigma").is_none());
    assert!(default_json.get("bk_mean_reversion").is_none());

    // schema-rejection-test: misspelled Black-Karasinski key
    let unknown = serde_json::json!({ "tree_model": "black_karasinski", "bk_kappa": 0.03 });
    assert!(serde_json::from_value::<ModelConfig>(unknown).is_err());

    for (sigma, kappa) in [
        (-0.20, 0.03),
        (0.20, -0.03),
        (f64::NAN, 0.03),
        (0.20, f64::NAN),
    ] {
        let invalid = ModelConfig {
            bk_sigma: Some(sigma),
            bk_mean_reversion: Some(kappa),
            ..ModelConfig::default()
        };
        assert!(invalid.validate().is_err(), "sigma={sigma}, kappa={kappa}");
    }
}

#[test]
fn market_quotes_reject_retired_clean_price_key() {
    // schema-rejection-test
    let err = serde_json::from_str::<MarketQuoteOverrides>(r#"{"quoted_clean_price": 99.5}"#)
        .expect_err("retired market_quotes key must be rejected");
    assert!(err.to_string().contains("unknown field"), "{err}");
    let quotes: MarketQuoteOverrides =
        serde_json::from_str(r#"{"quoted_clean_price_pct": 99.5, "quoted_premium": 12.5}"#)
            .expect("canonical market_quotes keys parse");
    assert_eq!(quotes.quoted_clean_price_pct, Some(99.5));
    assert_eq!(quotes.quoted_premium, Some(12.5));
}

#[test]
fn quoted_premium_is_an_iv_target_not_a_price_driver() {
    let quotes = MarketQuoteOverrides {
        quoted_clean_price_pct: Some(99.5),
        quoted_premium: Some(12.5),
        ..Default::default()
    };
    quotes
        .validate()
        .expect("a premium does not conflict with a price driver");
    assert!(quotes.has_price_driver());
    assert_eq!(quotes.required_quoted_premium().expect("premium"), 12.5);
    let missing = MarketQuoteOverrides::default()
        .required_quoted_premium()
        .expect_err("missing premium");
    assert!(
        missing
            .to_string()
            .contains("instrument_pricing_overrides.market_quotes.quoted_premium")
    );
}

#[test]
fn model_config_rejects_retired_short_rate_keys() {
    // schema-rejection-test
    for (retired, replacement) in [
        ("mean_reversion", "hw1f_mean_reversion"),
        ("hazard_volatility", "hazard_sigma"),
    ] {
        let json = format!(r#"{{"{retired}": 0.03}}"#);
        let err = serde_json::from_str::<ModelConfig>(&json)
            .expect_err("retired model_config key must be rejected");
        assert!(
            err.to_string().contains("unknown field"),
            "{retired}: unexpected error {err}"
        );
        let json = format!(r#"{{"{replacement}": 0.03}}"#);
        serde_json::from_str::<ModelConfig>(&json)
            .unwrap_or_else(|err| panic!("{replacement} must parse: {err}"));
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct FocusedWireFixture {
    id: String,
    #[serde(default, skip_serializing_if = "InstrumentPricingOverrides::is_empty")]
    instrument_pricing_overrides: InstrumentPricingOverrides,
    #[serde(default, skip_serializing_if = "MetricPricingOverrides::is_empty")]
    metric_pricing_overrides: MetricPricingOverrides,
    #[serde(default, skip_serializing_if = "ScenarioPricingOverrides::is_empty")]
    scenario_pricing_overrides: ScenarioPricingOverrides,
}

#[test]
fn bump_config_rejects_retired_twin_bump_keys() {
    // schema-rejection-test
    for retired in [
        "rho_bump_decimal",
        "vega_bump_decimal",
        "ytm_bump_decimal",
        "spot_bump_pct",
        "vol_bump_pct",
    ] {
        let json = format!(r#"{{"{retired}": 0.0001}}"#);
        let err = serde_json::from_str::<BumpConfig>(&json)
            .expect_err("retired bump key must be rejected");
        assert!(
            err.to_string().contains("unknown field"),
            "{retired}: unexpected error {err}"
        );
    }
    let current: BumpConfig = serde_json::from_str(r#"{"ytm_bump_bp": 1.0, "rate_bump_bp": 2.0}"#)
        .expect("canonical bump keys parse");
    assert_eq!(current.ytm_bump_bp, Some(1.0));
    assert_eq!(current.rate_bump_bp, Some(2.0));
}

#[test]
fn focused_categories_validate_independently() {
    let instrument = InstrumentPricingOverrides::default().with_quoted_clean_price_pct(100.0);
    let metrics = MetricPricingOverrides::default()
        .with_ytm_bump_bp(1.0)
        .with_spot_bump_decimal(0.01)
        .with_vol_bump_decimal(0.01)
        .with_rate_bump_bp(1.0);
    let scenario = ScenarioPricingOverrides::default().with_scenario_price_shock_decimal(-0.05);

    assert!(instrument.validate().is_ok());
    assert!(metrics.validate().is_ok());
    assert!(scenario.validate().is_ok());
    assert!(
        MetricPricingOverrides::default()
            .with_vol_bump_decimal(-0.01)
            .validate()
            .is_err()
    );
}

#[test]
fn instrument_vol_surface_extrapolation_roundtrips() {
    for policy in [
        VolSurfaceExtrapolation::Error,
        VolSurfaceExtrapolation::Clamp,
    ] {
        let mut overrides = InstrumentPricingOverrides::default();
        overrides.model_config.vol_surface_extrapolation = policy;
        let json = serde_json::to_string(&overrides).expect("serialize");
        let roundtrip: InstrumentPricingOverrides =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(roundtrip.model_config.vol_surface_extrapolation, policy);
    }
}

#[test]
fn focused_wire_uses_only_three_closed_canonical_fields() {
    let fixture = FocusedWireFixture {
        id: "fixture".to_string(),
        instrument_pricing_overrides: InstrumentPricingOverrides::default()
            .with_quoted_clean_price_pct(99.5),
        metric_pricing_overrides: MetricPricingOverrides::default()
            .with_theta_period(finstack_quant_core::dates::Tenor::weekly()),
        scenario_pricing_overrides: ScenarioPricingOverrides::default()
            .with_scenario_price_shock_decimal(-0.05),
    };

    let value = serde_json::to_value(&fixture).expect("serialize focused fixture");
    let object = value.as_object().expect("fixture object");
    assert!(object.contains_key("instrument_pricing_overrides"));
    assert!(object.contains_key("metric_pricing_overrides"));
    assert!(object.contains_key("scenario_pricing_overrides"));
    assert!(!object.contains_key("pricing_overrides"));

    let roundtrip: FocusedWireFixture =
        serde_json::from_value(value).expect("deserialize focused fixture");
    assert_eq!(
        roundtrip
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price_pct,
        Some(99.5)
    );

    for field in [
        "instrument_pricing_overrides",
        "metric_pricing_overrides",
        "scenario_pricing_overrides",
    ] {
        let mut invalid = serde_json::json!({"id": "fixture"});
        invalid[field] = serde_json::Value::Null;
        assert!(serde_json::from_value::<FocusedWireFixture>(invalid).is_err());
    }
    assert!(
        serde_json::from_value::<FocusedWireFixture>(serde_json::json!({
            "id": "fixture",
            "pricing_overrides": {}
        }))
        .is_err()
    );

    let schema =
        serde_json::to_value(schemars::schema_for!(FocusedWireFixture)).expect("serialize schema");
    let properties = schema["properties"].as_object().expect("schema properties");
    assert!(properties.contains_key("instrument_pricing_overrides"));
    assert!(properties.contains_key("metric_pricing_overrides"));
    assert!(properties.contains_key("scenario_pricing_overrides"));
    assert!(!properties.contains_key("pricing_overrides"));
}

#[test]
fn monte_carlo_accuracy_controls_validate() {
    let mut controls = InstrumentPricingOverrides::default()
        .with_mc_paths(10_000)
        .with_mc_antithetic(false);
    controls.model_config.mc_target_ci_half_width = Some(1.25);
    assert!(controls.validate().is_ok());
    assert_eq!(controls.model_config.mc_antithetic, Some(false));
    assert_eq!(controls.model_config.mc_target_ci_half_width, Some(1.25));

    for target in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let mut invalid = InstrumentPricingOverrides::default();
        invalid.model_config.mc_target_ci_half_width = Some(target);
        assert!(invalid.validate().is_err());
    }
}

#[test]
fn seed_scenario_lives_in_model_config() {
    // schema-rejection-test: `metric_pricing_overrides.mc_seed_scenario`
    let err = serde_json::from_str::<MetricPricingOverrides>(r#"{"mc_seed_scenario": "x"}"#)
        .expect_err("retired metric_pricing_overrides.mc_seed_scenario must be rejected");
    assert!(err.to_string().contains("unknown field"), "{err}");

    let config: ModelConfig = serde_json::from_str(r#"{"mc_seed_scenario": "stress"}"#)
        .expect("model_config.mc_seed_scenario parses");
    assert_eq!(config.mc_seed_scenario.as_deref(), Some("stress"));
    let overrides = InstrumentPricingOverrides::default().with_mc_seed_scenario("stress");
    assert_eq!(overrides.model_config, config);
}

#[test]
fn scenario_price_shock_is_a_decimal_key() {
    // schema-rejection-test: `scenario_pricing_overrides.scenario_price_shock_pct`
    let err =
        serde_json::from_str::<ScenarioPricingOverrides>(r#"{"scenario_price_shock_pct": -0.05}"#)
            .expect_err("retired scenario_price_shock_pct must be rejected");
    assert!(err.to_string().contains("unknown field"), "{err}");
    let current: ScenarioPricingOverrides =
        serde_json::from_str(r#"{"scenario_price_shock_decimal": -0.05}"#)
            .expect("canonical scenario key parses");
    assert_eq!(current.scenario_price_shock_decimal, Some(-0.05));
}

#[test]
fn cds_quantlib_day_count_flags_are_not_model_config_keys() {
    for retired in ["cds_aod_half_day_bias", "cds_act360_include_last_day"] {
        // schema-rejection-test: `model_config.cds_aod_half_day_bias` / `model_config.cds_act360_include_last_day`
        let json = format!(r#"{{"{retired}": true}}"#);
        let err = serde_json::from_str::<ModelConfig>(&json)
            .expect_err("retired CDS model_config flag must be rejected");
        assert!(err.to_string().contains("unknown field"), "{err}");
    }
}

#[test]
fn theta_period_is_a_tenor() {
    use finstack_quant_core::dates::Tenor;

    let overrides: MetricPricingOverrides =
        serde_json::from_str(r#"{"theta_period": {"count": 2, "unit": "weeks"}}"#)
            .expect("tenor theta_period parses");
    assert_eq!(
        overrides.theta_period,
        Some(Tenor::parse("2W").expect("tenor"))
    );
    // schema-rejection-test: the retired string form `"theta_period": "1W"`
    assert!(serde_json::from_str::<MetricPricingOverrides>(r#"{"theta_period": "1W"}"#).is_err());
    // A zero-length horizon is not a tenor.
    assert!(
        serde_json::from_str::<MetricPricingOverrides>(
            r#"{"theta_period": {"count": 0, "unit": "days"}}"#
        )
        .is_err()
    );
}

#[test]
fn theta_day_basis_is_a_metric_override() {
    let overrides: MetricPricingOverrides =
        serde_json::from_str(r#"{"theta_day_basis": "trading_252"}"#).expect("parses");
    assert_eq!(overrides.theta_day_basis, Some(ThetaDayBasis::Trading252));
    assert_eq!(overrides.theta_days_per_year(), 252.0);
    assert_eq!(
        MetricPricingOverrides::default().theta_days_per_year(),
        365.0
    );

    let mut leg = MetricPricingOverrides::default();
    leg.apply_defaults(&overrides);
    assert_eq!(leg.theta_day_basis, Some(ThetaDayBasis::Trading252));
}
