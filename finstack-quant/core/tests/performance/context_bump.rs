//! Frozen public-API numerical records for the CCM-I1 ownership investigation.
//!
//! The exporter compares complete deterministic state, hexadecimal float bits,
//! and Decimal mantissa/scale/currency with an optional C0 record. It exposes no
//! production counters and makes no allocation claim from Arc identities.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::bumps::{
    BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump,
};
use finstack_quant_core::market_data::context::{MarketContext, MarketContextState};
use finstack_quant_core::market_data::scalars::{
    MarketScalar, ScalarTimeSeries, SeriesInterpolation,
};
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{
    BaseCorrelationCurve, CreditIndexData, DiscountCurve, HazardCurve,
};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::{Error, InputError};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::io::Write;
use std::sync::Arc;
use time::{Duration, Month};

const EXPIRIES: [f64; 3] = [0.25, 1.0, 2.0];
const STRIKES: [f64; 3] = [80.0, 100.0, 120.0];
const VOLS: [f64; 9] = [0.18, 0.19, 0.20, 0.20, 0.21, 0.22, 0.22, 0.23, 0.24];

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).expect("fixture date")
}

fn bits(value: f64) -> String {
    format!("{:016x}", value.to_bits())
}

fn sha256(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut encoded, "{byte:02x}").expect("format SHA-256 byte");
    }
    encoded
}

fn encode_float_bits(value: &mut Value) {
    match value {
        Value::Number(number) if number.is_f64() => {
            *value = Value::String(bits(number.as_f64().expect("stored f64")));
        }
        Value::Array(values) => values.iter_mut().for_each(encode_float_bits),
        Value::Object(values) => values.values_mut().for_each(encode_float_bits),
        _ => {}
    }
}

fn snapshot(market: &MarketContext) -> Value {
    // MarketContextState sorts every domain identifier and uses BTreeMaps for
    // prices and issuer metadata. Float JSON values are converted before any
    // serialization so no u64 float bit pattern is emitted as a JSON number.
    let state = MarketContextState::try_from(market).expect("coherent state");
    let mut credit_bundles = json!(
        state
            .credit_indices
            .iter()
            .map(|entry| {
                let index = market
                    .get_credit_index(&entry.id)
                    .expect("captured credit index");
                json!({
                    "id": entry.id.as_str(),
                    "index_credit_curve": index.index_credit_curve(),
                    "base_correlation_curve": index.base_correlation_curve(),
                    "issuer_credit_curves": index.issuer_credit_curves(),
                })
            })
            .collect::<Vec<_>>()
    );
    encode_float_bits(&mut credit_bundles);
    let mut state = serde_json::to_value(state).expect("state serialization");
    encode_float_bits(&mut state);
    let mut prices: Vec<_> = market.prices_iter().collect();
    prices.sort_by(|a, b| a.0.cmp(b.0));
    let exact_prices: Vec<_> = prices
        .into_iter()
        .map(|(id, value)| match value {
            MarketScalar::Unitless(value) => {
                json!({"id": id.as_str(), "kind": "unitless", "bits": bits(*value)})
            }
            MarketScalar::Price(value) => json!({
                "id": id.as_str(), "kind": "money", "currency": value.currency().to_string(),
                "mantissa": value.amount_decimal().mantissa().to_string(),
                "scale": value.amount_decimal().scale(),
            }),
        })
        .collect();
    json!({"state": state, "exact_prices": exact_prices, "credit_bundles": credit_bundles})
}

fn discount_curve(id: &str) -> DiscountCurve {
    DiscountCurve::builder(id)
        .base_date(base_date())
        .knots((0..40).map(|i| {
            let time = f64::from(i) * 0.5;
            (time, (-0.04 * time).exp())
        }))
        .build()
        .expect("discount fixture")
}

fn surface(id: &str) -> VolSurface {
    VolSurface::from_grid(id, &EXPIRIES, &STRIKES, &VOLS, Default::default(), None)
        .expect("surface fixture")
}

fn series(id: &str) -> ScalarTimeSeries {
    ScalarTimeSeries::new(
        id,
        (0..256)
            .map(|i| (base_date() + Duration::days(i), 100.0 + 0.1 * i as f64))
            .collect(),
        Some(Currency::USD),
    )
    .expect("series fixture")
    .with_interpolation(SeriesInterpolation::Step)
}

fn fixture(unrelated_curves: usize) -> MarketContext {
    let mut market = MarketContext::new();
    for index in 0..unrelated_curves {
        market.insert_mut(discount_curve(&format!("CURVE_{index}")));
    }
    market.insert_surface_mut(surface("EQ-VOL"));
    market.insert_price_mut(
        "SPX-SPOT",
        MarketScalar::Price(Money::from((100_i64, Currency::USD))),
    );
    market.insert_series_mut(series("SPX-CLOSE"));
    market
}

fn additive(value: f64) -> BumpSpec {
    BumpSpec {
        mode: BumpMode::Additive,
        units: BumpUnits::Fraction,
        value,
        bump_type: BumpType::Parallel,
    }
}

fn generic(id: &str, spec: BumpSpec) -> MarketBump {
    MarketBump::Curve {
        id: CurveId::new(id),
        spec,
    }
}

fn bump_records(bumps: &[MarketBump]) -> Vec<Value> {
    bumps.iter().enumerate().map(|(index, bump)| match bump {
        MarketBump::Curve { id, spec } => json!({
            "index": index, "kind": "generic_curve", "id": id.as_str(),
            "mode": format!("{:?}", spec.mode), "units": format!("{:?}", spec.units),
            "value_bits": bits(spec.value), "bump_type": format!("{:?}", spec.bump_type),
        }),
        MarketBump::VolBucketPct { vol_surface_id, expiries, strikes, pct } => json!({
            "index": index, "kind": "typed_vol_bucket", "id": vol_surface_id.as_str(),
            "expiries": expiries.as_ref().map(|values| values.iter().map(|v| bits(*v)).collect::<Vec<_>>()),
            "strikes": strikes.as_ref().map(|values| values.iter().map(|v| bits(*v)).collect::<Vec<_>>()),
            "pct_bits": bits(*pct),
        }),
        MarketBump::BaseCorrBucketPts { surface_id, detachments, points } => json!({
            "index": index, "kind": "typed_base_correlation_bucket", "id": surface_id.as_str(),
            "detachments": detachments.as_ref().map(|values| values.iter().map(|v| bits(*v)).collect::<Vec<_>>()),
            "points_bits": bits(*points),
        }),
        MarketBump::FxPct { .. } => panic!("unexpected panel FX bump"),
    }).collect()
}

fn record(
    case_id: &str,
    base: &MarketContext,
    bumps: Vec<MarketBump>,
    family: &str,
) -> (MarketContext, Value) {
    let before = snapshot(base);
    let bumped = base.bump(bumps.clone()).expect(case_id);
    assert_eq!(snapshot(base), before, "source mutation in {case_id}");
    let output = snapshot(&bumped);
    let stats = base.stats();
    let payload = json!({
        "case_id": case_id, "resolved_family": family, "ordered_bumps": bump_records(&bumps),
        "consumer_route": "public MarketContext::bump; direct core API fixture",
        "reference_kind": "independent explicit operation oracle plus frozen complete C0 numeric and metadata records",
        "base_date": base_date().to_string(), "total_curve_count": stats.total_curves,
        "unrelated_curve_knots": 40,
        "store_counts": {"surfaces": stats.surface_count, "prices": stats.price_count, "series": stats.series_count, "credit_indices": stats.credit_index_count},
        "surface_grid_shape": [3, 3], "series_observation_count": base.get_series("SPX-CLOSE").expect("close fixture").len(),
        "source_unchanged": true,
        "base_sha256": sha256(&serde_json::to_vec(&before).expect("base bytes")),
        "output_sha256": sha256(&serde_json::to_vec(&output).expect("output bytes")),
        "base": before, "output": output,
    });
    (bumped, payload)
}

fn assert_other_content_unchanged(
    base: &MarketContext,
    bumped: &MarketContext,
    family: &str,
    id: &str,
) {
    let mut restored = bumped.clone();
    match family {
        "curve" => {
            restored.insert_mut(base.curve(id).expect("original curve").clone());
        }
        "surface" => {
            restored.insert_surface_mut((*base.get_surface(id).expect("original surface")).clone());
        }
        "price" => {
            restored.insert_price_mut(id, base.get_price(id).expect("original price").clone());
        }
        "series" => {
            restored.insert_series_mut(base.get_series(id).expect("original series").clone());
        }
        _ => panic!("unexpected family"),
    }
    assert_eq!(
        snapshot(&restored),
        snapshot(base),
        "unrelated data changed for {family}/{id}"
    );
}

fn assert_float(actual: f64, expected: f64) {
    assert!(actual.is_finite() && expected.is_finite());
    assert!((actual - expected).abs() <= 1e-12, "{actual} != {expected}");
}

fn assert_money(market: &MarketContext, id: &str, amount: &str) {
    let MarketScalar::Price(actual) = market.get_price(id).expect("money scalar") else {
        panic!("wrong scalar variant")
    };
    let expected = Money::from_decimal_str(amount, Currency::USD).expect("reference Decimal");
    assert_eq!(actual.currency(), Currency::USD);
    assert_eq!(actual.amount_decimal(), expected.amount_decimal());
}

fn ordinary_records() -> Vec<Value> {
    let mut records = Vec::new();
    for count in [8, 64, 256, 1024] {
        let base = fixture(count);
        let id = format!("context_bump_mixed/money_multiplier/curves_{count}");
        let (bumped, mut payload) = record(
            &id,
            &base,
            vec![generic("SPX-SPOT", BumpSpec::multiplier(1.01))],
            "price",
        );
        assert_money(&bumped, "SPX-SPOT", "101");
        assert_other_content_unchanged(&base, &bumped, "price", "SPX-SPOT");
        payload["unrelated_curve_count"] = json!(count);
        payload["role"] = json!(if count == 1024 {
            "large_context_stress_opportunity"
        } else {
            "opportunity"
        });
        records.push(payload);
    }
    let base = fixture(256);
    for (name, bump, typed) in [
        ("surface_abs", generic("EQ-VOL", additive(0.01)), false),
        (
            "typed_vol_bucket",
            MarketBump::VolBucketPct {
                vol_surface_id: CurveId::new("EQ-VOL"),
                expiries: None,
                strikes: None,
                pct: 5.0,
            },
            true,
        ),
    ] {
        let id = format!("context_bump_mixed/{name}/curves_256");
        let (bumped, mut payload) = record(&id, &base, vec![bump], "surface");
        let after = bumped.get_surface("EQ-VOL").expect("surface");
        assert_eq!(after.expiries(), EXPIRIES);
        assert_eq!(after.strikes(), STRIKES);
        for (&actual, original) in after.vols().iter().zip(VOLS) {
            let expected = if typed {
                original * (1.0 + 5.0 / 100.0)
            } else {
                original + 0.01
            };
            assert_eq!(actual.to_bits(), expected.to_bits());
        }
        assert_other_content_unchanged(&base, &bumped, "surface", "EQ-VOL");
        payload["role"] = json!(if typed {
            "unchanged_typed_route_control"
        } else {
            "opportunity"
        });
        payload["unrelated_curve_count"] = json!(256);
        records.push(payload);
    }
    let (bumped, mut payload) = record(
        "context_bump_mixed/series_multiplier/curves_256",
        &base,
        vec![generic("SPX-CLOSE", BumpSpec::multiplier(1.01))],
        "series",
    );
    let after = bumped.get_series("SPX-CLOSE").expect("series");
    assert_eq!(after.len(), 256);
    assert_eq!(after.currency(), Some(Currency::USD));
    assert_eq!(after.interpolation(), SeriesInterpolation::Step);
    for (index, (date, actual)) in after.observations().into_iter().enumerate() {
        assert_eq!(date, base_date() + Duration::days(index as i64));
        assert_eq!(
            actual.to_bits(),
            ((100.0 + 0.1 * index as f64) * 1.01).to_bits()
        );
    }
    assert_other_content_unchanged(&base, &bumped, "series", "SPX-CLOSE");
    payload["role"] = json!("direct_core_api_opportunity");
    payload["unrelated_curve_count"] = json!(256);
    payload["series_observation_count"] = json!(256);
    records.push(payload);

    let mut base = fixture(8);
    base.insert_price_mut("UNIT", MarketScalar::Unitless(100.0));
    let (bumped, payload) = record(
        "semantic/unitless_multiplier",
        &base,
        vec![generic("UNIT", BumpSpec::multiplier(1.01))],
        "price",
    );
    let MarketScalar::Unitless(actual) = bumped.get_price("UNIT").expect("unitless") else {
        panic!("wrong variant")
    };
    assert_eq!(actual.to_bits(), (100.0_f64 * 1.01).to_bits());
    assert_other_content_unchanged(&base, &bumped, "price", "UNIT");
    records.push(payload);
    let (bumped, payload) = record(
        "semantic/ordinary_curve_parallel_bp",
        &base,
        vec![generic("CURVE_0", BumpSpec::parallel_bp(1.0))],
        "curve",
    );
    let original = base.get_discount("CURVE_0").expect("original");
    let after = bumped.get_discount("CURVE_0").expect("after");
    for (&time, &df) in original.knots().iter().zip(original.dfs()) {
        assert_float(after.df(time), df * (-0.0001 * time).exp());
    }
    assert_other_content_unchanged(&base, &bumped, "curve", "CURVE_0");
    records.push(payload);
    records
}

fn precedence_records() -> Vec<Value> {
    let mut records = Vec::new();
    for (name, has_curve, has_surface, has_price, has_series, family, spec) in [
        (
            "all_families",
            true,
            true,
            true,
            true,
            "curve",
            BumpSpec::parallel_bp(1.0),
        ),
        (
            "curve_before_surface",
            true,
            true,
            false,
            false,
            "curve",
            BumpSpec::parallel_bp(1.0),
        ),
        (
            "surface_before_price",
            false,
            true,
            true,
            false,
            "surface",
            additive(0.01),
        ),
        (
            "price_before_series",
            false,
            false,
            true,
            true,
            "price",
            BumpSpec::multiplier(1.01),
        ),
    ] {
        let mut base = fixture(8);
        if has_curve {
            base.insert_mut(discount_curve("SHARED"));
        }
        if has_surface {
            base.insert_surface_mut(surface("SHARED"));
        }
        if has_price {
            base.insert_price_mut(
                "SHARED",
                MarketScalar::Price(Money::from((100_i64, Currency::USD))),
            );
        }
        if has_series {
            base.insert_series_mut(series("SHARED"));
        }
        let (bumped, payload) = record(
            &format!("semantic/precedence/{name}"),
            &base,
            vec![generic("SHARED", spec)],
            family,
        );
        match family {
            "curve" => assert_float(
                bumped.get_discount("SHARED").expect("curve").df(1.0),
                (-0.0401_f64).exp(),
            ),
            "surface" => {
                for (&actual, original) in bumped
                    .get_surface("SHARED")
                    .expect("surface")
                    .vols()
                    .iter()
                    .zip(VOLS)
                {
                    assert_eq!(actual.to_bits(), (original + 0.01).to_bits());
                }
            }
            "price" => assert_money(&bumped, "SHARED", "101"),
            _ => panic!("unexpected family"),
        }
        assert_other_content_unchanged(&base, &bumped, family, "SHARED");
        records.push(payload);
    }
    records
}

fn ordered_records() -> Vec<Value> {
    let base = fixture(8);
    let add = generic("SPX-SPOT", additive(10.0));
    let multiply = generic("SPX-SPOT", BumpSpec::multiplier(1.1));
    let mut records = Vec::new();
    for (name, bumps, expected) in [
        (
            "money_add_then_multiply",
            vec![add.clone(), multiply.clone()],
            "121",
        ),
        (
            "money_multiply_then_add",
            vec![multiply.clone(), add.clone()],
            "120",
        ),
    ] {
        let (bumped, payload) = record(&format!("semantic/order/{name}"), &base, bumps, "price");
        assert_money(&bumped, "SPX-SPOT", expected);
        assert_other_content_unchanged(&base, &bumped, "price", "SPX-SPOT");
        records.push(payload);
    }
    let (bumped, payload) = record(
        "semantic/order/interleaved_repeat",
        &base,
        vec![
            add,
            generic("EQ-VOL", additive(0.01)),
            generic("SPX-CLOSE", BumpSpec::multiplier(1.01)),
            multiply,
        ],
        "price_surface_series",
    );
    assert_money(&bumped, "SPX-SPOT", "121");
    for (&actual, original) in bumped
        .get_surface("EQ-VOL")
        .expect("surface")
        .vols()
        .iter()
        .zip(VOLS)
    {
        assert_eq!(actual.to_bits(), (original + 0.01).to_bits());
    }
    for ((date, actual), (original_date, original)) in bumped
        .get_series("SPX-CLOSE")
        .expect("series")
        .observations()
        .into_iter()
        .zip(
            base.get_series("SPX-CLOSE")
                .expect("original")
                .observations(),
        )
    {
        assert_eq!(date, original_date);
        assert_eq!(actual.to_bits(), (original * 1.01).to_bits());
    }
    let mut restored = bumped;
    restored.insert_price_mut(
        "SPX-SPOT",
        base.get_price("SPX-SPOT").expect("original").clone(),
    );
    restored.insert_surface_mut((*base.get_surface("EQ-VOL").expect("original")).clone());
    restored.insert_series_mut(base.get_series("SPX-CLOSE").expect("original").clone());
    assert_eq!(snapshot(&restored), snapshot(&base));
    records.push(payload);
    let (bumped, payload) = record(
        "semantic/surface_zero_floor",
        &base,
        vec![generic("EQ-VOL", additive(-0.3))],
        "surface",
    );
    assert!(
        bumped
            .get_surface("EQ-VOL")
            .expect("surface")
            .vols()
            .iter()
            .all(|value| value.to_bits() == 0.0_f64.to_bits())
    );
    assert_other_content_unchanged(&base, &bumped, "surface", "EQ-VOL");
    records.push(payload);
    let (bumped, payload) = record("semantic/empty_bumps", &base, Vec::new(), "none");
    assert_eq!(snapshot(&bumped), snapshot(&base));
    records.push(payload);
    records
}

fn error_record(
    case_id: &str,
    base: &MarketContext,
    bumps: Vec<MarketBump>,
    expected: &str,
) -> Value {
    let before = snapshot(base);
    let error = base.bump(bumps.clone()).expect_err(case_id);
    let (variant, target) = match &error {
        Error::Input(InputError::NotFound { id }) => ("Input::NotFound", Some(id.as_str())),
        Error::Input(InputError::UnsupportedBump { .. }) => ("Input::UnsupportedBump", None),
        Error::Input(InputError::ConversionOverflow) => ("Input::ConversionOverflow", None),
        Error::Validation(_) => ("Validation", None),
        _ => panic!("unfrozen public error variant: {error:?}"),
    };
    assert_eq!(variant, expected, "{case_id}: {error}");
    assert_eq!(snapshot(base), before, "failed bump mutated source");
    json!({
        "case_id": case_id, "role": "required_error_control", "ordered_bumps": bump_records(&bumps),
        "error_variant": variant, "error_target": target, "error_message": error.to_string(),
        "source_unchanged": true, "base": before,
    })
}

fn error_records() -> Vec<Value> {
    let base = fixture(8);
    let mut records = Vec::new();
    for (first, second) in [("ZZ-MISSING", "AA-MISSING"), ("AA-MISSING", "ZZ-MISSING")] {
        let payload = error_record(
            &format!("semantic/errors/first_missing_{first}"),
            &base,
            vec![
                generic("SPX-SPOT", BumpSpec::multiplier(1.01)),
                generic(first, additive(0.01)),
                generic(second, additive(0.01)),
            ],
            "Input::NotFound",
        );
        assert_eq!(payload["error_target"], json!(first));
        records.push(payload);
    }
    for (name, value) in [
        ("nan", f64::NAN),
        ("positive_infinity", f64::INFINITY),
        ("negative_infinity", f64::NEG_INFINITY),
    ] {
        records.push(error_record(
            &format!("semantic/errors/{name}"),
            &base,
            vec![generic("SPX-SPOT", BumpSpec::multiplier(value))],
            "Validation",
        ));
    }
    let mut collision = base.clone();
    collision.insert_series_mut(series("SPX-SPOT"));
    records.push(error_record(
        "semantic/errors/unsupported_price_must_not_fall_through_to_series",
        &collision,
        vec![generic("SPX-SPOT", BumpSpec::parallel_bp(1.0))],
        "Input::UnsupportedBump",
    ));
    records.push(error_record(
        "semantic/errors/unsupported_surface_key_rate",
        &base,
        vec![generic(
            "EQ-VOL",
            BumpSpec::triangular_key_rate_bp(0.25, 1.0, 2.0, 1.0),
        )],
        "Input::UnsupportedBump",
    ));
    let overflow = base
        .clone()
        .insert_price("OVERFLOW", MarketScalar::Unitless(f64::MAX));
    records.push(error_record(
        "semantic/errors/scalar_overflow",
        &overflow,
        vec![generic("OVERFLOW", BumpSpec::multiplier(2.0))],
        "Validation",
    ));
    let overflow = base.insert_price(
        "OVERFLOW",
        MarketScalar::Price(
            Money::from_decimal_str("10000000000000000000000000000", Currency::USD)
                .expect("large Money"),
        ),
    );
    records.push(error_record(
        "semantic/errors/money_overflow",
        &overflow,
        vec![generic("OVERFLOW", BumpSpec::multiplier(10.0))],
        "Input::ConversionOverflow",
    ));
    records
}

fn credit_fixture() -> MarketContext {
    let hazard = HazardCurve::flat("CDX-HZD", base_date(), 0.02, 0.4).expect("index hazard");
    let issuer = HazardCurve::flat("ISSUER-HZD", base_date(), 0.03, 0.4).expect("issuer hazard");
    let correlation = BaseCorrelationCurve::builder("CDX-BC")
        .knots([(3.0, 0.2), (7.0, 0.4), (10.0, 0.5)])
        .build()
        .expect("correlation");
    let index = CreditIndexData::builder()
        .num_constituents(1)
        .recovery_rate(0.4)
        .index_credit_curve(Arc::new(hazard.clone()))
        .base_correlation_curve(Arc::new(correlation.clone()))
        .issuer_curves([("Issuer".to_string(), Arc::new(issuer.clone()))])
        .build()
        .expect("credit index");
    fixture(8)
        .insert(hazard)
        .insert(issuer)
        .insert(correlation)
        .insert_credit_index("CDX", index)
        .expect("canonical index")
}

fn credit_records() -> Vec<Value> {
    let base = credit_fixture();
    let mut records = Vec::new();
    let bumps = vec![
        generic("CDX-HZD", BumpSpec::parallel_bp(6.0)),
        generic("ISSUER-HZD", BumpSpec::parallel_bp(12.0)),
        generic("CDX-BC", additive(0.01)),
        generic("CDX-HZD", BumpSpec::parallel_bp(3.0)),
    ];
    let (bumped, mut payload) = record(
        "semantic/credit/ordered_dependency_rebind",
        &base,
        bumps,
        "credit_curves",
    );
    let index = bumped.get_credit_index("CDX").expect("rebound index");
    let hazard = bumped.get_hazard("CDX-HZD").expect("canonical hazard");
    let issuer = bumped.get_hazard("ISSUER-HZD").expect("canonical issuer");
    let correlation = bumped
        .get_base_correlation("CDX-BC")
        .expect("canonical correlation");
    assert!(Arc::ptr_eq(index.index_credit_curve(), &hazard));
    assert!(Arc::ptr_eq(index.base_correlation_curve(), &correlation));
    assert!(Arc::ptr_eq(
        index
            .issuer_credit_curves()
            .expect("issuers")
            .get("Issuer")
            .expect("issuer"),
        &issuer
    ));
    let mut observations = Vec::new();
    for time in [0.5, 1.0, 3.0, 5.0, 10.0] {
        // Generic RateBp hazard bumps are par-spread shifts divided by LGD.
        assert_float(hazard.hazard_rate(time), 0.02 + 9.0 * 1e-4 / 0.6);
        assert_float(issuer.hazard_rate(time), 0.03 + 12.0 * 1e-4 / 0.6);
        assert_eq!(
            index.index_credit_curve().sp(time).to_bits(),
            hazard.sp(time).to_bits()
        );
        assert_eq!(
            index.get_issuer_curve("Issuer").sp(time).to_bits(),
            issuer.sp(time).to_bits()
        );
        observations.push(json!({"time_bits": bits(time), "hazard_bits": bits(hazard.hazard_rate(time)), "issuer_bits": bits(issuer.hazard_rate(time)), "survival_bits": bits(hazard.sp(time)), "issuer_survival_bits": bits(issuer.sp(time)), "bundled_survival_bits": bits(index.index_credit_curve().sp(time)), "bundled_issuer_survival_bits": bits(index.get_issuer_curve("Issuer").sp(time))}));
    }
    for (&actual, original) in correlation.correlations().iter().zip([0.2_f64, 0.4, 0.5]) {
        assert_eq!(actual.to_bits(), (original + 0.01).to_bits());
    }
    payload["credit_observations"] = json!(observations);
    payload["credit_canonical_arc_identity"] = json!(true);
    records.push(payload);
    let (bumped, mut payload) = record(
        "semantic/credit/noncurve_preserves_dependencies",
        &base,
        vec![generic("SPX-SPOT", BumpSpec::multiplier(1.01))],
        "price",
    );
    assert!(Arc::ptr_eq(
        &base.get_credit_index("CDX").expect("base index"),
        &bumped.get_credit_index("CDX").expect("same index")
    ));
    assert_other_content_unchanged(&base, &bumped, "price", "SPX-SPOT");
    payload["credit_index_arc_shared"] = json!(true);
    records.push(payload);
    let (bumped, mut payload) = record(
        "semantic/credit/typed_base_correlation_control",
        &base,
        vec![MarketBump::BaseCorrBucketPts {
            surface_id: CurveId::new("CDX-BC"),
            detachments: Some(vec![7.0]),
            points: 0.01,
        }],
        "base_correlation",
    );
    let direct = bumped.get_base_correlation("CDX-BC").expect("correlation");
    let index = bumped.get_credit_index("CDX").expect("rebound index");
    assert!(Arc::ptr_eq(index.base_correlation_curve(), &direct));
    for (&actual, expected) in direct.correlations().iter().zip([0.2_f64, 0.41, 0.5]) {
        assert_float(actual, expected);
    }
    payload["credit_canonical_arc_identity"] = json!(true);
    records.push(payload);
    records
}

#[test]
#[ignore = "CCM numerical export; run explicitly before matched benchmarks"]
fn export_context_bump_reference() {
    let mut records = ordinary_records();
    records.extend(precedence_records());
    records.extend(ordered_records());
    records.extend(error_records());
    records.extend(credit_records());
    assert_eq!(records.len(), 30);
    let payload = json!({
        "schema": "finstack_quant.ccm_context_bump_reference/1",
        "float_encoding": "16-digit hexadecimal f64 to_bits strings",
        "comparison_policy": {"ownership_changes": "exact complete records including f64 bits and Decimal mantissa/scale/currency", "independent_formula_absolute_tolerance": 1e-12},
        "diagnostics": {"owning_map_sharing": null, "allocations": null, "copied_bytes": null, "limitation": "Owning maps are private; this public-API panel reports no map-sharing or allocation measurement. Per-object Arc equality only verifies dependency identity."},
        "test_source_sha256": sha256(include_bytes!("context_bump.rs")),
        "benchmark_source_sha256": sha256(include_bytes!("../../benches/context_bump.rs")),
        "records": records,
    });
    if let Some(path) = std::env::var_os("CCM_CORE_REFERENCE_FILE") {
        let reference: Value =
            serde_json::from_slice(&std::fs::read(path).expect("read frozen C0 reference"))
                .expect("C0 JSON");
        assert_eq!(
            payload["records"], reference["records"],
            "ownership candidate differs from frozen C0 numeric/Decimal/error records"
        );
    }
    if let Some(directory) = std::env::var_os("CCM_CORE_EXPORT_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).expect("create export directory");
        let bytes = serde_json::to_vec_pretty(&payload).expect("reference JSON");
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join("context-bump-reference.json"))
            .expect("create new immutable reference");
        output.write_all(&bytes).expect("write reference");
    }
}
