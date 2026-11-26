use super::super::ModelKey;
use super::dispatch::*;
use crate::instruments::Instrument;
use crate::instruments::envelope::MAX_JSON_BYTES;
use crate::instruments::json_loader::{InstrumentEnvelope, InstrumentJson};
use crate::metrics::MetricId;
use crate::metrics::risk::MarketHistory;
use crate::results::ValuationResult;
use finstack_quant_core::Error;
use finstack_quant_core::contract::LoadLimits;
use finstack_quant_core::market_data::context::MarketContext;
use serde_json::Value;
use std::sync::Arc;

fn instrument_json_for_pricing(
    instrument_json: &str,
    metric_pricing_overrides: Option<&str>,
) -> finstack_quant_core::Result<String> {
    validate_instrument_json(instrument_json, metric_pricing_overrides)
}
use crate::instruments::commodity::commodity_spot_option::CommoditySpotOption;
use crate::instruments::credit_derivatives::cds_option::CdsOption;
use crate::instruments::equity::equity_option::EquityOption;
use crate::instruments::equity::private_markets_fund::PrivateMarketsFund;
use crate::instruments::fixed_income::bond::Bond;
use crate::instruments::fixed_income::cmo::AgencyCmo;
use crate::instruments::fixed_income::convertible::ConvertibleBond;
use crate::instruments::fixed_income::revolving_credit::RevolvingCredit;
use crate::instruments::fixed_income::structured_credit::StructuredCredit;
use crate::instruments::fixed_income::term_loan::TermLoan;
use crate::instruments::fx::fx_option::FxOption;
use crate::instruments::rates::ir_future::InterestRateFuture;
use crate::instruments::rates::swaption::Swaption;
use crate::metrics::{MetricCalculator, MetricContext, MetricRegistry};
use crate::pricer::{InstrumentType, Pricer, PricerKey, PricerRegistry, PricingError, expect_inst};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use std::sync::atomic::{AtomicUsize, Ordering};

fn price_instrument_from_json(
    instrument_json: &str,
    market: &MarketContext,
    as_of: &str,
    model: &str,
    metrics: &[String],
    metric_pricing_overrides_json: Option<&str>,
    market_history: Option<&MarketHistory>,
) -> finstack_quant_core::Result<ValuationResult> {
    let instrument =
        parse_boxed_instrument_from_json(instrument_json, metric_pricing_overrides_json, None)?;
    price_instrument(
        &instrument,
        market,
        as_of,
        model,
        metrics,
        market_history,
        crate::instruments::PricingOptions::default(),
    )
}

fn envelope_value(instrument: InstrumentJson) -> Value {
    serde_json::to_value(InstrumentEnvelope::new(instrument)).expect("serialize envelope")
}

fn envelope_json(instrument: InstrumentJson) -> String {
    serde_json::to_string(&InstrumentEnvelope::new(instrument)).expect("serialize envelope")
}

fn bond_instrument_json() -> String {
    let bond = Bond::fixed(
        "TEST-BOND",
        Money::from((1_000_000_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 1)
            .expect("date"),
        finstack_quant_core::dates::Date::from_calendar_date(2034, time::Month::January, 1)
            .expect("date"),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    envelope_json(InstrumentJson::Bond(bond))
}

fn market_context() -> MarketContext {
    let base = finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 1)
        .expect("date");
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([(0.5, 0.99), (1.0, 0.98), (5.0, 0.90), (10.0, 0.80)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("curve");
    MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
}

struct FixedJsonBondPricer {
    calls: Arc<AtomicUsize>,
}

impl Pricer for FixedJsonBondPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Bond, ModelKey::Discounting)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        _market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let bond = expect_inst::<Bond>(instrument, InstrumentType::Bond)?;
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ValuationResult::stamped(
            bond.id(),
            as_of,
            Money::from((321_i64, bond.notional.currency())),
        ))
    }
}

struct RepriceJsonMetric;

impl MetricCalculator for RepriceJsonMetric {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        context.reprice_raw(context.curves.as_ref(), context.as_of)
    }
}

#[test]
fn default_model_resolves_to_instrument_native_model() {
    let bond = Bond::fixed(
        "TEST-BOND",
        Money::from((1_000_000_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 1)
            .expect("date"),
        finstack_quant_core::dates::Date::from_calendar_date(2034, time::Month::January, 1)
            .expect("date"),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    assert_eq!(
        resolve_model_key(&bond, "default").expect("model"),
        ModelKey::Discounting
    );

    let fx_option = FxOption::example().expect("fx option");
    assert_eq!(
        resolve_model_key(&fx_option, "default").expect("model"),
        ModelKey::Black76
    );

    let equity_option = EquityOption::example().expect("equity option");
    let swaption = Swaption::example().expect("example");
    let commodity_spot_option = CommoditySpotOption::example().expect("example");
    for instrument in [
        &equity_option as &dyn Instrument,
        &swaption as &dyn Instrument,
        &commodity_spot_option as &dyn Instrument,
    ] {
        assert_eq!(
            resolve_model_key(instrument, "default").expect("model"),
            ModelKey::Black76
        );
    }

    let mut normal_swaption = Swaption::example().expect("example");
    normal_swaption.vol_model = crate::instruments::rates::VolatilityModel::Normal;
    assert_eq!(
        resolve_model_key(&normal_swaption, "default").expect("model"),
        ModelKey::Normal
    );

    let convertible = ConvertibleBond::example().expect("convertible");
    assert_eq!(
        resolve_model_key(&convertible, "default").expect("model"),
        ModelKey::Tree
    );
}

fn equity_option_json_with_negative_vol_override() -> String {
    let option = EquityOption::example().expect("option");
    let mut json = envelope_value(InstrumentJson::EquityOption(option));
    json["instrument"]["spec"]["instrument_pricing_overrides"]["market_quotes"]["implied_volatility"] =
        Value::from(-0.20);
    serde_json::to_string(&json).expect("serialize")
}

fn equity_option_json_with_invalid_strike() -> String {
    let mut option = EquityOption::example().expect("option");
    option.strike = -100.0;
    envelope_json(InstrumentJson::EquityOption(option))
}

fn fx_spot_spec_value() -> Value {
    serde_json::json!({
        "id": "EURUSD-SPOT",
        "base_currency": "EUR",
        "quote_currency": "USD",
        "settlement_date": "2025-01-17",
        "quoted_spot": 1.20,
        "notional": {"amount": "1000000", "currency": "EUR"},
        "attributes": {},
    })
}

#[test]
fn instrument_envelope_from_spec_wraps_bare_fx_spec() {
    let envelope =
        instrument_envelope_from_spec("fx_spot", fx_spot_spec_value()).expect("canonical fx spot");
    assert_eq!(envelope.instrument.type_tag(), "fx_spot");
    let parsed = serde_json::to_value(&envelope).expect("json");
    assert_eq!(
        parsed["schema"],
        crate::instruments::envelope::InstrumentSchema::CURRENT.as_str()
    );
    assert_eq!(parsed["instrument"]["type"], "fx_spot");
    assert_eq!(parsed["instrument"]["spec"]["id"], "EURUSD-SPOT");
}

#[test]
fn instrument_envelope_from_spec_rejects_tagged_payload() {
    let err = instrument_envelope_from_spec(
        "fx_forward",
        serde_json::json!({"type": "fx_spot", "spec": fx_spot_spec_value()}),
    )
    .expect_err("tagged payload should be rejected");
    assert!(
        err.to_string()
            .contains("constructor requires a bare spec object")
    );
}

#[test]
fn validate_typed_instrument_json_rejects_other_envelope_type() {
    let fx_spot = serde_json::to_string(
        &instrument_envelope_from_spec("fx_spot", fx_spot_spec_value()).expect("canonical fx spot"),
    )
    .expect("json");
    let err = validate_typed_instrument_json("fx_forward", &fx_spot)
        .expect_err("wrong envelope type should be rejected");
    assert!(
        err.to_string()
            .contains("expected instrument type `fx_forward`, got `fx_spot`")
    );
}

#[test]
fn parse_typed_instrument_json_returns_the_concrete_type() {
    let fx_spot = serde_json::to_string(
        &instrument_envelope_from_spec("fx_spot", fx_spot_spec_value()).expect("canonical fx spot"),
    )
    .expect("json");
    let spot: crate::instruments::fx::fx_spot::FxSpot =
        parse_typed_instrument_json(&fx_spot).expect("typed fx spot");
    assert_eq!(spot.id.as_str(), "EURUSD-SPOT");
    let err =
        parse_typed_instrument_json::<crate::instruments::fx::fx_forward::FxForward>(&fx_spot)
            .expect_err("wrong envelope type should be rejected");
    assert!(matches!(err, Error::Validation(_)));
    assert!(
        err.to_string()
            .contains("expected instrument type `fx_forward`, got `fx_spot`")
    );
}

#[test]
fn instrument_json_for_pricing_error_includes_instrument_id() {
    // Malformed metric_pricing_overrides on a well-formed instrument JSON.
    let json = bond_instrument_json();
    let err = instrument_json_for_pricing(&json, Some("not-valid-json"))
        .expect_err("malformed metric_pricing_overrides must error");
    let msg = err.to_string();
    assert!(
        msg.contains("invalid metric_pricing_overrides JSON"),
        "expected metric_pricing_overrides error, got: {msg}"
    );
    assert!(
        msg.contains("[instrument=TEST-BOND]"),
        "expected instrument id suffix, got: {msg}"
    );
}

#[test]
fn instrument_json_for_pricing_error_without_id_when_json_unparseable() {
    // Instrument JSON itself is malformed, so id cannot be extracted; the
    // error message should still be useful but without an [instrument=...]
    // suffix.
    let err = instrument_json_for_pricing("{not-json", Some("{}"))
        .expect_err("malformed instrument JSON must error");
    let msg = err.to_string();
    assert!(
        !msg.contains("[instrument="),
        "no id should be attached when JSON is unparseable, got: {msg}"
    );
}

#[test]
fn instrument_json_for_pricing_merges_metric_overrides() {
    let json = bond_instrument_json();
    let merged = instrument_json_for_pricing(
        &json,
        Some(
            r#"{"theta_period":{"count":1,"unit":"days"},"breakeven_config":{"target":"z_spread","mode":"linear"}}"#,
        ),
    )
    .expect("merge");
    let parsed: Value = serde_json::from_str(&merged).expect("json");
    assert_eq!(
        parsed["instrument"]["spec"]["metric_pricing_overrides"]["theta_period"],
        serde_json::json!({"count": 1, "unit": "days"})
    );
    assert_eq!(
        parsed["instrument"]["spec"]["metric_pricing_overrides"]["breakeven_config"]["target"],
        "z_spread"
    );
}

#[test]
fn partial_metric_patch_preserves_absent_fields_and_nested_bumps() {
    let mut document: Value = serde_json::from_str(&bond_instrument_json()).expect("json");
    document["instrument"]["spec"]["metric_pricing_overrides"] = serde_json::json!({
        "theta_period": {"count": 1, "unit": "weeks"},
        "bump_config": {"rate_bump_bp": 2.0, "vol_bump_decimal": 0.02, "adaptive_bumps": true}
    });
    let json = document.to_string();
    for patch in ["{}", r#"{"theta_day_basis":"trading_252"}"#] {
        let merged = instrument_json_for_pricing(&json, Some(patch)).expect("merge");
        let parsed: Value = serde_json::from_str(&merged).expect("json");
        let overrides = &parsed["instrument"]["spec"]["metric_pricing_overrides"];
        assert_eq!(
            overrides["theta_period"],
            document["instrument"]["spec"]["metric_pricing_overrides"]["theta_period"]
        );
        assert_eq!(
            overrides["bump_config"],
            document["instrument"]["spec"]["metric_pricing_overrides"]["bump_config"]
        );
    }

    let merged = instrument_json_for_pricing(
        &json,
        Some(r#"{"bump_config":{"rate_bump_bp":3.0},"theta_period":null}"#),
    )
    .expect("merge");
    let parsed: Value = serde_json::from_str(&merged).expect("json");
    let overrides = &parsed["instrument"]["spec"]["metric_pricing_overrides"];
    assert!(overrides["theta_period"].is_null());
    assert_eq!(overrides["bump_config"]["rate_bump_bp"], 3.0);
    assert_eq!(overrides["bump_config"]["vol_bump_decimal"], 0.02);
    assert_eq!(overrides["bump_config"]["adaptive_bumps"], true);
}

#[test]
fn composite_metric_pricing_overrides_merge_into_root() {
    let composite =
        crate::instruments::composite::CompositeInstrument::example().expect("composite example");
    let json = envelope_json(InstrumentJson::Composite(Box::new(composite)));
    let merged = instrument_json_for_pricing(
        &json,
        Some(r#"{"theta_period":{"count":1,"unit":"months"}}"#),
    )
    .expect("merge");
    let envelope: crate::instruments::json_loader::InstrumentEnvelope =
        serde_json::from_str(&merged).expect("merged composite must still parse");
    let InstrumentJson::Composite(parsed) = envelope.instrument else {
        panic!("expected composite");
    };
    assert_eq!(
        parsed.metric_pricing_overrides.theta_period,
        Some(finstack_quant_core::dates::Tenor::monthly())
    );
}

#[test]
fn structured_credit_tranche_metric_overrides_merge_via_typed_path() {
    let mut tranche =
        crate::instruments::fixed_income::structured_credit::StructuredCreditTranche::example()
            .expect("structured credit tranche example");
    tranche
        .get_metric_pricing_overrides_mut()
        .expect("tranche metric overrides")
        .bump_config
        .vol_bump_decimal = Some(0.02);
    let json = envelope_json(InstrumentJson::StructuredCreditTranche(Box::new(tranche)));
    let patch = r#"{"theta_period":{"count":1,"unit":"weeks"},"bump_config":{"rate_bump_bp":3.0}}"#;

    let validated = validate_instrument_json(&json, Some(patch)).expect("validate tranche");
    let envelope: InstrumentEnvelope =
        serde_json::from_str(&validated).expect("parse validated tranche");
    let InstrumentJson::StructuredCreditTranche(parsed_tranche) = envelope.instrument else {
        panic!("expected structured credit tranche");
    };
    assert_eq!(
        parsed_tranche.deal().metric_pricing_overrides.theta_period,
        Some(finstack_quant_core::dates::Tenor::weekly())
    );
    assert_eq!(
        parsed_tranche
            .deal()
            .metric_pricing_overrides
            .bump_config
            .rate_bump_bp,
        Some(3.0)
    );
    assert_eq!(
        parsed_tranche
            .deal()
            .metric_pricing_overrides
            .bump_config
            .vol_bump_decimal,
        Some(0.02)
    );

    let boxed = parse_boxed_instrument_from_json(&json, Some(patch), None).expect("parse boxed");
    let controls = boxed
        .as_instrument()
        .get_metric_pricing_overrides()
        .expect("tranche controls");
    assert_eq!(
        controls.theta_period,
        Some(finstack_quant_core::dates::Tenor::weekly())
    );
    assert_eq!(controls.bump_config.rate_bump_bp, Some(3.0));
    assert_eq!(controls.bump_config.vol_bump_decimal, Some(0.02));
}

#[test]
fn structured_credit_tranche_corrupt_wire_metric_overrides_repaired_by_patch() {
    let tranche =
        crate::instruments::fixed_income::structured_credit::StructuredCreditTranche::example()
            .expect("structured credit tranche example");
    let mut document = envelope_value(InstrumentJson::StructuredCreditTranche(Box::new(tranche)));
    document["instrument"]["spec"]["deal"]["metric_pricing_overrides"] = serde_json::json!({
        "theta_period": "invalid",
        "bump_config": {"vol_bump_decimal": 0.02}
    });
    let json = document.to_string();

    assert!(validate_instrument_json(&json, None).is_err());

    let patch = r#"{"theta_period":{"count":1,"unit":"weeks"}}"#;
    let validated = validate_instrument_json(&json, Some(patch))
        .expect("call-time patch should repair corrupt tranche deal metric overrides");
    let parsed: Value = serde_json::from_str(&validated).expect("canonical tranche json");
    assert_eq!(
        parsed["instrument"]["spec"]["deal"]["metric_pricing_overrides"]["theta_period"],
        serde_json::json!({"count": 1, "unit": "weeks"})
    );
    assert_eq!(
        parsed["instrument"]["spec"]["deal"]["metric_pricing_overrides"]["bump_config"]["vol_bump_decimal"],
        0.02
    );

    let boxed = parse_boxed_instrument_from_json(&json, Some(patch), None)
        .expect("parse_boxed should repair corrupt tranche deal metric overrides");
    let controls = boxed
        .as_instrument()
        .get_metric_pricing_overrides()
        .expect("tranche controls");
    assert_eq!(
        controls.theta_period,
        Some(finstack_quant_core::dates::Tenor::weekly())
    );
    assert_eq!(controls.bump_config.vol_bump_decimal, Some(0.02));
}

#[test]
fn validate_instrument_json_rejects_invalid_pricing_overrides() {
    let err = validate_instrument_json(&equity_option_json_with_negative_vol_override(), None)
        .expect_err("negative implied volatility override must be rejected");
    assert!(
        err.to_string().contains("NegativeValue") || err.to_string().contains("negative"),
        "unexpected error: {err}"
    );
}

#[test]
fn parse_boxed_instrument_from_json_rejects_invalid_pricing_overrides() {
    let Err(err) = parse_boxed_instrument_from_json(
        &equity_option_json_with_negative_vol_override(),
        None,
        None,
    ) else {
        panic!("negative implied volatility override must be rejected")
    };
    assert!(
        err.to_string().contains("NegativeValue") || err.to_string().contains("negative"),
        "unexpected error: {err}"
    );
}

#[test]
fn validate_instrument_json_rejects_domain_invariants() {
    let err = validate_instrument_json(&equity_option_json_with_invalid_strike(), None)
        .expect_err("negative equity-option strike must be rejected");
    assert!(
        err.to_string().contains("strike") && err.to_string().contains("positive"),
        "unexpected error: {err}"
    );
}

#[test]
fn validate_instrument_json_rejects_credit_and_convertible_invariants() {
    let mut cds_option = CdsOption::example().expect("CDS option");
    cds_option.exercise_settlement_date = Some(cds_option.expiry - time::Duration::days(1));
    let json = envelope_json(InstrumentJson::CdsOption(cds_option));
    assert!(
        validate_instrument_json(&json, None)
            .expect_err("exercise settlement before expiry must fail")
            .to_string()
            .contains("on or after legal expiry")
    );

    let mut convertible = ConvertibleBond::example().expect("convertible");
    convertible.conversion.ratio = None;
    convertible.conversion.price = None;
    let json = envelope_json(InstrumentJson::ConvertibleBond(convertible));
    assert!(
        validate_instrument_json(&json, None)
            .expect_err("missing conversion terms must fail")
            .to_string()
            .contains("conversion.ratio")
    );
}

#[test]
fn validate_instrument_json_rejects_invalid_revolving_credit_path() {
    let mut facility = RevolvingCredit::example().expect("revolving credit");
    facility.draw_repay_spec =
        crate::instruments::fixed_income::revolving_credit::DrawRepaySpec::Deterministic(vec![
            crate::instruments::fixed_income::revolving_credit::DrawRepayEvent {
                date: facility.maturity + time::Duration::days(1),
                amount: Money::from((1_000_000_i64, Currency::USD)),
                is_draw: true,
            },
        ]);
    let json = envelope_json(InstrumentJson::RevolvingCredit(facility));
    assert!(
        validate_instrument_json(&json, None)
            .expect_err("post-maturity draw must fail")
            .to_string()
            .contains("maturity")
    );
}

#[test]
fn validate_instrument_json_rejects_rates_and_securitized_invariants() {
    let mut future = InterestRateFuture::example().expect("IR future");
    future.contract_specs.convexity_adjustment = None;
    future.vol_surface_id = None;
    let json = envelope_json(InstrumentJson::InterestRateFuture(future));
    assert!(
        validate_instrument_json(&json, None)
            .expect_err("missing convexity source must fail")
            .to_string()
            .contains("convexity_adjustment")
    );

    let mut cmo = AgencyCmo::example().expect("CMO");
    cmo.reference_tranche_id = "MISSING".to_string();
    let json = envelope_json(InstrumentJson::AgencyCmo(cmo));
    assert!(
        validate_instrument_json(&json, None)
            .expect_err("unknown reference tranche must fail")
            .to_string()
            .contains("reference tranche")
    );
}

#[test]
fn validate_instrument_json_rejects_invalid_term_loan_notional() {
    let mut loan = TermLoan::example().expect("term loan");
    loan.notional_limit = Money::from((-1_i64, Currency::USD));
    let json = envelope_json(InstrumentJson::TermLoan(loan));
    assert!(
        validate_instrument_json(&json, None)
            .expect_err("negative notional must fail")
            .to_string()
            .contains("notional_limit")
    );
}

#[test]
fn validate_instrument_json_rejects_non_conserving_waterfall() {
    let fund = PrivateMarketsFund::example().expect("fund");
    let mut json = envelope_value(InstrumentJson::PrivateMarketsFund(fund));
    json["instrument"]["spec"]["waterfall_spec"]["tranches"][3]["promote_tier"]["gp_share"] =
        Value::from(0.6);
    let json = serde_json::to_string(&json).expect("json");

    validate_instrument_json(&json, None)
        .expect_err("LP and GP shares above 100% must be rejected");
}

#[test]
fn validate_instrument_json_rejects_invalid_cleanup_call_threshold() {
    let mut deal = StructuredCredit::example().expect("example");
    deal.cleanup_call_decimal = Some(-0.5);
    let json = envelope_json(InstrumentJson::StructuredCredit(Box::new(deal)));

    let err = validate_instrument_json(&json, None)
        .expect_err("cleanup-call threshold outside (0, 1) must be rejected");
    assert!(err.to_string().contains("cleanup_call_decimal"));
}

#[test]
fn validate_instrument_json_accepts_versioned_envelope() {
    let bond = Bond::fixed(
        "ENVELOPE-BOND",
        Money::from((1_000_000_i64, Currency::USD)),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 1)
            .expect("date"),
        finstack_quant_core::dates::Date::from_calendar_date(2034, time::Month::January, 1)
            .expect("date"),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    let envelope = InstrumentEnvelope::new(InstrumentJson::Bond(bond));
    let json = serde_json::to_string(&envelope).expect("envelope json");

    let canonical = validate_instrument_json(&json, None).expect("valid envelope");
    let value: Value = serde_json::from_str(&canonical).expect("canonical json");
    assert_eq!(
        value["schema"],
        crate::instruments::envelope::InstrumentSchema::CURRENT.as_str()
    );
    assert_eq!(value["instrument"]["type"], "bond");
}

#[test]
fn validate_instrument_json_merges_overrides_before_validation() {
    let mut document: Value = serde_json::from_str(&bond_instrument_json()).expect("fixture");
    document["instrument"]["spec"]["metric_pricing_overrides"] =
        serde_json::json!({ "theta_period": "invalid" });
    let json = document.to_string();

    assert!(validate_instrument_json(&json, None).is_err());
    let prepared = validate_instrument_json(
        &json,
        Some(r#"{"theta_period":{"count":1,"unit":"weeks"}}"#),
    )
    .expect("merged before validation");
    let value: Value = serde_json::from_str(&prepared).expect("canonical");
    assert_eq!(
        value["instrument"]["spec"]["metric_pricing_overrides"]["theta_period"],
        serde_json::json!({"count": 1, "unit": "weeks"})
    );
    assert_eq!(
        validate_instrument_json(&prepared, None).expect("valid"),
        prepared
    );
    assert!(
        validate_instrument_json(&json, Some("{"))
            .expect_err("bad options")
            .to_string()
            .contains("invalid metric_pricing_overrides JSON")
    );
}

#[test]
fn native_and_wire_preparation_merge_and_validate_identically() {
    let envelope =
        InstrumentEnvelope::decode_bytes(bond_instrument_json().as_bytes(), &LoadLimits::default())
            .expect("fixture");
    let InstrumentJson::Bond(mut bond) = envelope.instrument else {
        panic!("bond fixture")
    };
    bond.metric_pricing_overrides = serde_json::from_value(serde_json::json!({
        "theta_period": {"count": 1, "unit": "weeks"},
        "bump_config": {"rate_bump_bp": -2.0, "vol_bump_decimal": 0.02}
    }))
    .expect("stored controls");
    assert!(ParsedInstrument::from_instrument(Box::new(bond.clone()), None, None).is_err());
    let wire = envelope_json(InstrumentJson::Bond(bond.clone()));
    let patch = r#"{"bump_config":{"rate_bump_bp":3.0},"theta_period":null}"#;
    let native = ParsedInstrument::from_instrument(Box::new(bond), Some(patch), None)
        .expect("repair before validation");
    let decoded =
        parse_boxed_instrument_from_json(&wire, Some(patch), None).expect("same wire preparation");
    let native_controls = native
        .as_instrument()
        .get_metric_pricing_overrides()
        .expect("controls");
    let wire_controls = decoded
        .as_instrument()
        .get_metric_pricing_overrides()
        .expect("controls");
    assert_eq!(
        serde_json::to_value(native_controls).expect("serde"),
        serde_json::to_value(wire_controls).expect("serde")
    );
    assert_eq!(native_controls.bump_config.rate_bump_bp, Some(3.0));
    assert_eq!(native_controls.bump_config.vol_bump_decimal, Some(0.02));
    assert_eq!(native_controls.theta_period, None);
    let market = market_context();
    let as_of = "2025-01-01";
    let metrics = vec!["theta".to_owned(), "dv01".to_owned()];
    let native_result = price_instrument(
        &native,
        &market,
        as_of,
        "default",
        &metrics,
        None,
        Default::default(),
    )
    .expect("native price");
    let wire_result = price_instrument(
        &decoded,
        &market,
        as_of,
        "default",
        &metrics,
        None,
        Default::default(),
    )
    .expect("wire price");
    assert_eq!(native_result.value, wire_result.value);
    assert_eq!(native_result.measures, wire_result.measures);
    assert!(native_result.measures.contains_key("theta_period_days"));
}

#[test]
fn instrument_inventory_parses_envelopes_in_order_and_shares_error_text() {
    let json = bond_instrument_json();
    let inventory =
        parse_boxed_instruments_from_json(&format!("[{json},{json}]")).expect("inventory");
    assert_eq!(inventory.len(), 2);
    assert_eq!(inventory[0].id(), "TEST-BOND");
    assert!(
        parse_boxed_instruments_from_json("[]")
            .expect("empty inventory")
            .is_empty()
    );

    let bare: Value = serde_json::from_str::<Value>(&json).expect("json")["instrument"].clone();
    let single = parse_boxed_instrument_from_json(&bare.to_string(), None, None)
        .err()
        .expect("bare instrument must be rejected")
        .to_string();
    let many = parse_boxed_instruments_from_json(&format!("[{bare}]"))
        .err()
        .expect("bare instrument must be rejected")
        .to_string();
    assert!(
        single.contains("invalid instrument envelope JSON"),
        "{single}"
    );
    assert!(many.contains("invalid instrument envelope JSON"), "{many}");

    let oversized = format!("[{}]", " ".repeat(MAX_JSON_BYTES));
    let error = parse_boxed_instruments_from_json(&oversized)
        .err()
        .expect("oversized inventory must be rejected")
        .to_string();
    assert!(error.contains("input exceeds limit: bytes"), "{error}");
}

#[test]
fn parse_instrument_from_json_accepts_envelope_and_enforces_size_cap() {
    let json = bond_instrument_json();
    assert!(matches!(
        parse_instrument_from_json(&json).expect("envelope payload"),
        InstrumentJson::Bond(_)
    ));

    let mut invalid: Value = serde_json::from_str(&json).expect("fixture");
    invalid["instrument"]["spec"]["retired_field"] = Value::Bool(false);
    let error = parse_instrument_from_json(&invalid.to_string()).expect_err("unknown field");
    assert!(error.to_string().contains("retired_field"), "{error}");
    let syntax = parse_instrument_from_json("{").expect_err("invalid JSON");
    assert!(syntax.to_string().contains("EOF"), "{syntax}");

    let oversized = " ".repeat(MAX_JSON_BYTES + 1);
    let error = parse_instrument_from_json(&oversized).expect_err("oversized payload fails");
    assert!(
        error.to_string().contains("input exceeds limit: bytes"),
        "{error}"
    );
}

#[test]
fn parse_boxed_instrument_from_json_rejects_domain_invariants() {
    let Err(err) =
        parse_boxed_instrument_from_json(&equity_option_json_with_invalid_strike(), None, None)
    else {
        panic!("negative equity-option strike must be rejected")
    };
    assert!(
        err.to_string().contains("strike") && err.to_string().contains("positive"),
        "unexpected error: {err}"
    );
}

#[test]
fn json_pricing_routes_validate_instrument_before_model_resolution() {
    let invalid = equity_option_json_with_invalid_strike();
    let market = MarketContext::new();

    let plain = price_instrument_from_json(
        &invalid,
        &market,
        "not-a-date",
        "not-a-model",
        &[],
        None,
        None,
    )
    .expect_err("instrument validation must win");
    let with_metrics = price_instrument_from_json(
        &invalid,
        &market,
        "not-a-date",
        "not-a-model",
        &["not-a-metric".to_string()],
        None,
        None,
    )
    .expect_err("instrument validation must win");
    for error in [plain, with_metrics] {
        let message = error.to_string();
        assert!(
            message.contains("strike") && message.contains("positive"),
            "unexpected error ordering: {message}"
        );
    }
}

#[test]
fn price_instrument_from_json_prices_bond() {
    let result = price_instrument_from_json(
        &bond_instrument_json(),
        &market_context(),
        "2024-01-01",
        "discounting",
        &[],
        None,
        None,
    )
    .expect("price");
    assert_eq!(result.instrument_id, "TEST-BOND");
}

#[test]
fn json_pricing_preserves_custom_registry_for_base_and_metric_repricing() {
    let parsed = parse_boxed_instrument_from_json(&bond_instrument_json(), None, None)
        .expect("validated bond JSON");
    let calls = Arc::new(AtomicUsize::new(0));
    let mut pricers = PricerRegistry::new();
    pricers
        .register(FixedJsonBondPricer {
            calls: Arc::clone(&calls),
        })
        .expect("unique custom pricer");
    let metric_id = MetricId::custom("json_registry_reprice");
    let mut metrics = MetricRegistry::new();
    metrics
        .register_metric(
            metric_id.clone(),
            Arc::new(RepriceJsonMetric),
            &[InstrumentType::Bond],
        )
        .expect("unique custom metric");

    let result = price_instrument(
        &parsed,
        &MarketContext::new(),
        "2024-01-01",
        "discounting",
        &[metric_id.to_string()],
        None,
        crate::instruments::PricingOptions::default()
            .with_registry(Arc::new(pricers))
            .with_metric_registry(Arc::new(metrics)),
    )
    .expect("custom JSON registry must drive base and nested repricing");

    assert_eq!(result.value.amount(), 321.0);
    assert_eq!(result.measures.get(&metric_id), Some(&321.0));
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "base PV and metric reprice must use the same custom registry"
    );
}

#[test]
fn price_instrument_from_json_accepts_metric_pricing_overrides() {
    let result = price_instrument_from_json(
        &bond_instrument_json(),
        &market_context(),
        "2024-01-01",
        "discounting",
        &["dirty_price".to_string()],
        Some(r#"{"theta_period":{"count":1,"unit":"days"}}"#),
        None,
    )
    .expect("price");
    assert_eq!(result.instrument_id, "TEST-BOND");
}

#[test]
fn json_pricing_accepts_registered_custom_term_loan_metrics() {
    let loan = TermLoan::example().expect("term loan");
    let json = envelope_json(InstrumentJson::TermLoan(loan));
    let result = price_instrument_from_json(
        &json,
        &market_context(),
        "2024-01-01",
        "discounting",
        &["all_in_rate".to_string(), "yt2y".to_string()],
        None,
        None,
    )
    .expect("registered custom metrics must cross the JSON boundary");

    assert!(result.measures.contains_key("all_in_rate"));
    assert!(result.measures.contains_key("yt2y"));
}

#[test]
fn revolving_credit_custom_metrics_use_as_of_drawn_balance() {
    use finstack_quant_cashflows::builder::RateSpec;

    // `drawn` (10M of 50M) is the balance at the valuation date;
    // the example's 5M draw on 2024-03-01 lies in the future and must not
    // enter the as-of metrics.
    let mut facility = RevolvingCredit::example().expect("revolving credit");
    facility.rate = RateSpec::Fixed { rate: 0.05 };
    let json = envelope_json(InstrumentJson::RevolvingCredit(facility));
    let result = price_instrument_from_json(
        &json,
        &market_context(),
        "2024-02-01",
        "discounting",
        &[
            "utilization_rate".to_string(),
            "available_capacity".to_string(),
        ],
        None,
        None,
    )
    .expect("registered revolving-credit metrics must cross the JSON boundary");

    assert_eq!(result.metric_str("utilization_rate"), Some(0.20));
    assert_eq!(result.metric_str("available_capacity"), Some(40_000_000.0));
}

#[test]
fn price_instrument_from_json_rejects_unknown_metric_names() {
    let err = price_instrument_from_json(
        &bond_instrument_json(),
        &market_context(),
        "2024-01-01",
        "discounting",
        &["dvO1".to_string()],
        None,
        None,
    )
    .expect_err("JSON pricing boundary should parse requested metrics strictly");

    assert!(
        err.to_string().contains("dvO1") || err.to_string().contains("dvo1"),
        "unknown metric error should include the requested metric, got: {err}"
    );
}

#[test]
fn price_instrument_from_json_accepts_market_history_for_hvar() {
    let history = crate::metrics::risk::MarketHistory::new(
        finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 1)
            .expect("date"),
        2,
        vec![
            crate::metrics::risk::MarketScenario::new(
                finstack_quant_core::dates::Date::from_calendar_date(
                    2023,
                    time::Month::December,
                    29,
                )
                .expect("date"),
                vec![crate::metrics::risk::RiskFactorShift {
                    factor: crate::metrics::risk::RiskFactorType::DiscountRate {
                        curve_id: finstack_quant_core::types::CurveId::new("USD-OIS"),
                        tenor_years: 5.0,
                        grid_years: crate::metrics::STANDARD_BUCKETS_YEARS.to_vec(),
                    },
                    shift: 0.0005,
                }],
            ),
            crate::metrics::risk::MarketScenario::new(
                finstack_quant_core::dates::Date::from_calendar_date(
                    2023,
                    time::Month::December,
                    28,
                )
                .expect("date"),
                vec![crate::metrics::risk::RiskFactorShift {
                    factor: crate::metrics::risk::RiskFactorType::DiscountRate {
                        curve_id: finstack_quant_core::types::CurveId::new("USD-OIS"),
                        tenor_years: 10.0,
                        grid_years: crate::metrics::STANDARD_BUCKETS_YEARS.to_vec(),
                    },
                    shift: -0.0003,
                }],
            ),
        ],
    );
    let result = price_instrument_from_json(
        &bond_instrument_json(),
        &market_context(),
        "2024-01-01",
        "discounting",
        &["hvar".to_string(), "expected_shortfall".to_string()],
        None,
        Some(&history),
    )
    .expect("HVar should price when market history is supplied");

    assert!(result.measures.contains_key(MetricId::HVar.as_str()));
    assert!(
        result
            .measures
            .contains_key(MetricId::ExpectedShortfall.as_str())
    );
}

#[test]
fn price_instrument_returns_requested_present_metrics() {
    let json = bond_instrument_json();
    let result = price_instrument_from_json(
        &json,
        &market_context(),
        "2024-01-01",
        "discounting",
        &["dirty_price".to_string(), "vega".to_string()],
        None,
        None,
    )
    .expect("metrics");
    let dirty_price = result
        .metric_str("dirty_price")
        .expect("requested metric is present");
    assert!(dirty_price.is_finite());
    assert_eq!(result.metric_str("vega"), Some(0.0));
}
