//! Serialization tests for market data types.
//!
//! This module contains serde roundtrip tests for market data types
//! that don't have their own dedicated test modules, as well as
//! cross-cutting serialization tests like MarketContext.
//!
//! Note: Curve-specific serde tests are in their respective modules
//! (curves/discount.rs, curves/forward.rs, etc.)

use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::context::MarketContextState;
use finstack_core::market_data::dividends::DividendSchedule;
use finstack_core::market_data::hierarchy::MarketDataHierarchy;
use finstack_core::market_data::scalars::{InflationIndex, InflationInterpolation, InflationLag};
use finstack_core::market_data::scalars::{ScalarTimeSeries, SeriesInterpolation};
use finstack_core::market_data::surfaces::VolSurface;
use finstack_core::market_data::term_structures::BaseCorrelationCurve;
use finstack_core::market_data::term_structures::CreditIndexData;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::market_data::term_structures::ForwardCurve;
use finstack_core::market_data::term_structures::HazardCurve;
use finstack_core::money::fx::FxQuery;
use finstack_core::money::fx::{FxConfig, FxMatrix, SimpleFxProvider};
use finstack_core::money::Money;
use finstack_core::types::CurveId;
use std::sync::Arc;
use time::Month;

fn test_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 15).unwrap()
}

// =============================================================================
// VolSurface Tests
// =============================================================================

#[test]
fn vol_surface_roundtrip() {
    let surface = VolSurface::builder("EQ-VOL")
        .expiries(&[1.0, 2.0])
        .strikes(&[90.0, 100.0, 110.0])
        .row(&[0.2, 0.21, 0.22])
        .row(&[0.19, 0.2, 0.21])
        .build()
        .unwrap();

    let json = serde_json::to_string_pretty(&surface).unwrap();
    let deserialized: VolSurface = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id(), surface.id());
    assert_eq!(deserialized.expiries(), surface.expiries());
    assert_eq!(deserialized.strikes(), surface.strikes());
    assert_eq!(deserialized.grid_shape(), surface.grid_shape());
}

// =============================================================================
// ScalarTimeSeries Tests
// =============================================================================

#[test]
fn scalar_time_series_roundtrip() {
    let d1 = Date::from_calendar_date(2024, Month::January, 31).unwrap();
    let d2 = Date::from_calendar_date(2024, Month::February, 29).unwrap();

    let series = ScalarTimeSeries::new("VOL-TS", vec![(d1, 0.2), (d2, 0.25)], None)
        .unwrap()
        .with_interpolation(SeriesInterpolation::Linear);

    let json = serde_json::to_string_pretty(&series).unwrap();
    let deserialized: ScalarTimeSeries = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id(), series.id());
    assert_eq!(deserialized.observations(), series.observations());
}

// =============================================================================
// InflationIndex Tests
// =============================================================================

#[test]
fn inflation_index_roundtrip() {
    let d1 = Date::from_calendar_date(2024, Month::January, 31).unwrap();
    let d2 = Date::from_calendar_date(2024, Month::February, 29).unwrap();

    let index = InflationIndex::new("US-CPI", vec![(d1, 300.0), (d2, 301.5)], Currency::USD)
        .unwrap()
        .with_interpolation(InflationInterpolation::Linear)
        .with_lag(InflationLag::Months(3));

    let json = serde_json::to_string_pretty(&index).unwrap();
    let deserialized: InflationIndex = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id, index.id);
    assert_eq!(deserialized.currency, index.currency);
    assert_eq!(deserialized.interpolation, index.interpolation);
    assert_eq!(deserialized.lag(), index.lag());
}

// =============================================================================
// MarketContext Tests
// =============================================================================

#[test]
fn market_context_roundtrip() {
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(test_date())
        .knots([(0.0, 1.0), (5.0, 0.9)])
        .build()
        .unwrap();

    let forward = ForwardCurve::builder("USD-SOFR", 0.25)
        .base_date(test_date())
        .knots([(0.0, 0.03), (5.0, 0.04)])
        .build()
        .unwrap();

    let surface = VolSurface::builder("VOL")
        .expiries(&[1.0, 2.0])
        .strikes(&[90.0, 100.0])
        .row(&[0.2, 0.2])
        .row(&[0.2, 0.2])
        .build()
        .unwrap();

    let dividends =
        DividendSchedule::new("AAPL-DIVS").add_cash(test_date(), Money::new(1.0, Currency::USD));

    let fx_provider = Arc::new(SimpleFxProvider::new());
    fx_provider
        .set_quote(Currency::USD, Currency::EUR, 1.1)
        .expect("valid test quote");
    let fx = FxMatrix::try_with_config(fx_provider, FxConfig::default()).expect("valid FxConfig");

    let ctx = MarketContext::new()
        .insert(discount)
        .insert(forward)
        .insert_surface(surface)
        .insert_dividends(dividends)
        .insert_fx(fx);

    let json = serde_json::to_string_pretty(&ctx).unwrap();
    let deserialized: MarketContext = serde_json::from_str(&json).unwrap();

    assert!(deserialized.get_discount("USD-OIS").is_ok());
    assert!(deserialized.get_forward("USD-SOFR").is_ok());
    assert!(deserialized.get_surface("VOL").is_ok());
    assert!(deserialized.get_dividend_schedule("AAPL-DIVS").is_ok());
    assert!(deserialized.fx().is_some());
}

#[test]
fn market_context_roundtrip_preserves_hierarchy() {
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(test_date())
        .knots([(0.0, 1.0), (5.0, 0.9)])
        .build()
        .unwrap();

    let hierarchy = MarketDataHierarchy::builder()
        .add_node("Rates/USD/OIS")
        .curve_ids(&["USD-OIS"])
        .add_node("Rates/EUR/ESTR")
        .curve_ids(&["EUR-ESTR"])
        .build()
        .unwrap();

    let mut ctx = MarketContext::new().insert(discount);
    ctx.set_hierarchy(hierarchy);

    let json = serde_json::to_string_pretty(&ctx).unwrap();
    let deserialized: MarketContext = serde_json::from_str(&json).unwrap();

    let report = deserialized
        .completeness_report()
        .expect("hierarchy should survive MarketContext serde round-trip");
    assert_eq!(report.missing.len(), 1);
    assert_eq!(report.missing[0].1, CurveId::from("EUR-ESTR"));
}

#[test]
fn market_context_v1_snapshot_without_hierarchy_restores_with_none() {
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(test_date())
        .knots([(0.0, 1.0), (5.0, 0.9)])
        .build()
        .unwrap();

    let state: MarketContextState = (&MarketContext::new().insert(discount)).into();
    let mut json = serde_json::to_value(state).unwrap();
    let object = json
        .as_object_mut()
        .expect("MarketContextState should serialize to a JSON object");
    object.insert("version".into(), serde_json::json!(1));
    object.remove("hierarchy");

    let restored: MarketContext = serde_json::from_value(json).unwrap();
    assert!(restored.get_discount("USD-OIS").is_ok());
    assert!(restored.completeness_report().is_none());
}

#[test]
fn market_context_restore_uses_quote_only_fx_snapshot() {
    let fx_provider = Arc::new(SimpleFxProvider::new());
    let fx = FxMatrix::try_with_config(
        fx_provider,
        FxConfig {
            enable_triangulation: true,
            pivot_currency: Currency::USD,
            ..Default::default()
        },
    )
    .expect("valid FxConfig");
    fx.set_quote(Currency::USD, Currency::EUR, 1.10)
        .expect("valid test quote");
    fx.set_quote(Currency::USD, Currency::GBP, 0.80)
        .expect("valid test quote");

    let ctx = MarketContext::new().insert_fx(fx);
    let restored: MarketContext =
        serde_json::from_str(&serde_json::to_string(&ctx).unwrap()).unwrap();
    let restored_fx = restored.fx().expect("restored snapshot should contain FX");

    let direct = restored_fx
        .rate(FxQuery::new(Currency::USD, Currency::EUR, test_date()))
        .expect("captured direct quote should restore");
    assert!((direct.rate - 1.10).abs() < 1e-12);

    let reciprocal = restored_fx
        .rate(FxQuery::new(Currency::EUR, Currency::USD, test_date()))
        .expect("reciprocal of captured quote should restore");
    assert!((reciprocal.rate - (1.0 / 1.10)).abs() < 1e-12);

    let triangulated = restored_fx
        .rate(FxQuery::new(Currency::EUR, Currency::GBP, test_date()))
        .expect("restored snapshot should triangulate from captured quotes");
    assert!((triangulated.rate - (1.0 / 1.10) * 0.80).abs() < 1e-12);
}

#[test]
fn market_context_state_is_deterministically_sorted_and_roundtrips_full_snapshot() {
    use finstack_core::market_data::scalars::MarketScalar;
    use finstack_core::money::Money;

    let d = test_date();

    // Intentionally insert in "unsorted" order to verify deterministic state ordering.
    let discount_b = DiscountCurve::builder("B-DISC")
        .base_date(d)
        .knots([(0.0, 1.0), (5.0, 0.9)])
        .build()
        .unwrap();
    let discount_a = DiscountCurve::builder("A-DISC")
        .base_date(d)
        .knots([(0.0, 1.0), (5.0, 0.92)])
        .build()
        .unwrap();
    let hazard = HazardCurve::builder("CDX-HAZ")
        .base_date(d)
        .knots([(1.0, 0.01), (5.0, 0.02)])
        .build()
        .unwrap();
    let base_corr = BaseCorrelationCurve::builder("CDX-BC")
        .knots([(3.0, 0.25), (7.0, 0.4)])
        .build()
        .unwrap();

    // Add issuer curves/weights/recoveries to cover optional-map serde paths.
    let mut issuer_curves = finstack_core::HashMap::default();
    issuer_curves.insert("ISSUER1".to_string(), std::sync::Arc::new(hazard.clone()));
    let issuer2_haz = HazardCurve::builder("ISSUER2-HAZ")
        .base_date(d)
        .knots([(1.0, 0.02), (5.0, 0.03)])
        .build()
        .unwrap();
    issuer_curves.insert(
        "ISSUER2".to_string(),
        std::sync::Arc::new(issuer2_haz.clone()),
    );
    let mut issuer_recovery = finstack_core::HashMap::default();
    issuer_recovery.insert("ISSUER1".to_string(), 0.35);
    let mut issuer_weights = finstack_core::HashMap::default();
    issuer_weights.insert("ISSUER1".to_string(), 0.6);
    issuer_weights.insert("ISSUER2".to_string(), 0.4);

    let credit_index = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.4)
        .index_credit_curve(std::sync::Arc::new(hazard.clone()))
        .base_correlation_curve(std::sync::Arc::new(base_corr.clone()))
        .issuer_curves(issuer_curves)
        .issuer_recovery_rates(issuer_recovery)
        .issuer_weights(issuer_weights)
        .build()
        .unwrap();

    let series = ScalarTimeSeries::new("TS", vec![(d, 1.0)], None)
        .unwrap()
        .with_interpolation(SeriesInterpolation::Linear);

    let surface = VolSurface::builder("VOL")
        .expiries(&[1.0, 2.0])
        .strikes(&[90.0, 100.0])
        .row(&[0.2, 0.2])
        .row(&[0.2, 0.2])
        .build()
        .unwrap();

    let ctx = MarketContext::new()
        .insert(discount_b)
        .insert(discount_a)
        .insert(hazard)
        .insert(issuer2_haz)
        .insert(base_corr)
        .insert_credit_index("CDX", credit_index)
        .insert_series(series)
        .insert_surface(surface)
        .insert_price(
            "EQ-SPOT",
            MarketScalar::Price(Money::new(100.0, Currency::USD)),
        )
        .map_collateral("USD-CSA", CurveId::from("A-DISC"));

    let state = MarketContextState::from(&ctx);

    // Curves are sorted by id in the state representation.
    let ids: Vec<String> = state
        .curves
        .iter()
        .map(|c| match c {
            finstack_core::market_data::context::CurveState::Discount(dc) => dc.id().to_string(),
            finstack_core::market_data::context::CurveState::Forward(fc) => fc.id().to_string(),
            finstack_core::market_data::context::CurveState::Hazard(hc) => hc.id().to_string(),
            finstack_core::market_data::context::CurveState::Inflation(ic) => ic.id().to_string(),
            finstack_core::market_data::context::CurveState::BaseCorrelation(bc) => {
                bc.id().to_string()
            }
            finstack_core::market_data::context::CurveState::VolIndex(vc) => vc.id().to_string(),
            finstack_core::market_data::context::CurveState::Price(pc) => pc.id().to_string(),
            finstack_core::market_data::context::CurveState::BasisSpread(bs) => bs.id().to_string(),
            finstack_core::market_data::context::CurveState::Parametric(pc) => pc.id().to_string(),
        })
        .collect();
    assert_eq!(
        ids,
        vec!["A-DISC", "B-DISC", "CDX-BC", "CDX-HAZ", "ISSUER2-HAZ"]
    );

    // Full JSON roundtrip should preserve ability to resolve referenced curves
    let json = serde_json::to_string_pretty(&ctx).unwrap();
    let roundtripped: MarketContext = serde_json::from_str(&json).unwrap();

    assert!(roundtripped.get_discount("A-DISC").is_ok());
    assert!(roundtripped.get_discount("B-DISC").is_ok());
    assert!(roundtripped.get_hazard("CDX-HAZ").is_ok());
    assert!(roundtripped.get_base_correlation("CDX-BC").is_ok());
    assert!(roundtripped.get_credit_index("CDX").is_ok());
    assert!(roundtripped.get_series("TS").is_ok());
    assert!(roundtripped.get_surface("VOL").is_ok());
    assert!(roundtripped.get_price("EQ-SPOT").is_ok());
    assert!(roundtripped.get_collateral("USD-CSA").is_ok());
}

#[test]
fn curve_storage_roundtrip_and_market_context_state_error_branch() {
    use finstack_core::market_data::context::{CurveState, CurveStorage};

    // CurveStorage roundtrip through serde (uses CurveState internally)
    let storage = CurveStorage::from(
        DiscountCurve::builder("USD-OIS")
            .base_date(test_date())
            .knots([(0.0, 1.0), (1.0, 0.98)])
            .build()
            .unwrap(),
    );
    let json = serde_json::to_string(&storage).unwrap();
    let de: CurveStorage = serde_json::from_str(&json).unwrap();
    assert_eq!(de.id().as_str(), "USD-OIS");

    // Also exercise CurveState serde directly
    let st = CurveState::Discount(
        DiscountCurve::builder("X")
            .base_date(test_date())
            .knots([(0.0, 1.0), (1.0, 0.99)])
            .build()
            .unwrap(),
    );
    let json = serde_json::to_string(&st).unwrap();
    let _back: CurveState = serde_json::from_str(&json).unwrap();

    // MarketContextState -> MarketContext error branch: credit index references missing curve IDs.
    let bad_state = MarketContextState {
        version: finstack_core::market_data::context::MARKET_CONTEXT_STATE_VERSION,
        curves: vec![],
        fx: None,
        surfaces: vec![],
        prices: std::collections::BTreeMap::new(),
        series: vec![],
        inflation_indices: vec![],
        dividends: vec![],
        credit_indices: vec![finstack_core::market_data::context::CreditIndexState {
            id: "CDX".to_string(),
            num_constituents: 125,
            recovery_rate: 0.4,
            index_credit_curve_id: "MISSING-HAZ".to_string(),
            base_correlation_curve_id: "MISSING-BC".to_string(),
            issuer_credit_curve_ids: None,
            issuer_recovery_rates: None,
            issuer_weights: None,
        }],
        fx_delta_vol_surfaces: vec![],
        vol_cubes: vec![],
        collateral: std::collections::BTreeMap::new(),
        hierarchy: None,
    };
    assert!(MarketContext::try_from(bad_state).is_err());
}

#[test]
fn curve_state_and_storage_roundtrip_all_variants() {
    use finstack_core::market_data::context::{CurveState, CurveStorage};
    use finstack_core::market_data::term_structures::{
        BaseCorrelationCurve, DiscountCurve, ForwardCurve, HazardCurve, InflationCurve,
    };

    let d = test_date();

    let disc = DiscountCurve::builder("DISC")
        .base_date(d)
        .knots([(0.0, 1.0), (1.0, 0.99)])
        .build()
        .unwrap();
    let fwd = ForwardCurve::builder("FWD", 0.25)
        .base_date(d)
        .knots([(0.0, 0.02), (1.0, 0.03)])
        .build()
        .unwrap();
    let haz = HazardCurve::builder("HAZ")
        .base_date(d)
        .knots([(1.0, 0.01), (5.0, 0.02)])
        .build()
        .unwrap();
    let inf = InflationCurve::builder("INF")
        .base_cpi(100.0)
        .base_date(d)
        .knots([(0.0, 100.0), (1.0, 101.0)])
        .build()
        .unwrap();
    let bc = BaseCorrelationCurve::builder("BC")
        .knots([(3.0, 0.25), (7.0, 0.4)])
        .build()
        .unwrap();

    let states = vec![
        CurveState::Discount(disc.clone()),
        CurveState::Forward(fwd.clone()),
        CurveState::Hazard(haz.clone()),
        CurveState::Inflation(inf.clone()),
        CurveState::BaseCorrelation(bc.clone()),
    ];
    for st in states {
        let json = serde_json::to_string(&st).unwrap();
        let _back: CurveState = serde_json::from_str(&json).unwrap();
    }

    let storages: Vec<CurveStorage> =
        vec![disc.into(), fwd.into(), haz.into(), inf.into(), bc.into()];
    for s in storages {
        let json = serde_json::to_string(&s).unwrap();
        let back: CurveStorage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id().as_str(), s.id().as_str());
    }
}

#[test]
fn market_context_state_roundtrip_hits_more_state_serde_lines() {
    use finstack_core::market_data::scalars::MarketScalar;
    use finstack_core::market_data::term_structures::{
        BaseCorrelationCurve, CreditIndexData, DiscountCurve, ForwardCurve, HazardCurve,
        InflationCurve,
    };
    use finstack_core::money::Money;
    use std::sync::Arc;

    let d = test_date();

    // Curves of all variants (exercise CurveState enum coverage)
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(d)
        .knots([(0.0, 1.0), (5.0, 0.9)])
        .build()
        .unwrap();
    let fwd = ForwardCurve::builder("USD-SOFR", 0.25)
        .base_date(d)
        .knots([(0.0, 0.03), (5.0, 0.04)])
        .build()
        .unwrap();
    let haz = HazardCurve::builder("CDX-HAZ")
        .base_date(d)
        .knots([(1.0, 0.01), (5.0, 0.02)])
        .build()
        .unwrap();
    let issuer_haz = HazardCurve::builder("ISSUER-HAZ")
        .base_date(d)
        .knots([(1.0, 0.02), (5.0, 0.03)])
        .build()
        .unwrap();
    let inf = InflationCurve::builder("US-CPI-CURVE")
        .base_cpi(100.0)
        .base_date(d)
        .knots([(0.0, 100.0), (1.0, 101.0)])
        .build()
        .unwrap();
    let bc = BaseCorrelationCurve::builder("CDX-BC")
        .knots([(3.0, 0.25), (7.0, 0.4)])
        .build()
        .unwrap();

    let mut issuer_curves = finstack_core::HashMap::default();
    issuer_curves.insert("ISSUER".to_string(), Arc::new(issuer_haz.clone()));

    let credit_index = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.4)
        .index_credit_curve(Arc::new(haz.clone()))
        .base_correlation_curve(Arc::new(bc.clone()))
        .issuer_curves(issuer_curves)
        .build()
        .unwrap();

    // Inflation index (separate from inflation curve)
    let idx = InflationIndex::new("US-CPI", vec![(d, 300.0)], Currency::USD)
        .unwrap()
        .with_interpolation(InflationInterpolation::Linear);

    let series = ScalarTimeSeries::new("TS", vec![(d, 1.0)], None)
        .unwrap()
        .with_interpolation(SeriesInterpolation::Linear);

    let surface = VolSurface::builder("VOL")
        .expiries(&[1.0, 2.0])
        .strikes(&[90.0, 100.0])
        .row(&[0.2, 0.2])
        .row(&[0.2, 0.2])
        .build()
        .unwrap();

    let ctx = MarketContext::new()
        .insert(disc)
        .insert(fwd)
        .insert(haz)
        .insert(issuer_haz)
        .insert(inf)
        .insert(bc)
        .insert_credit_index("CDX", credit_index)
        .insert_inflation_index("US-CPI", idx)
        .insert_series(series)
        .insert_surface(surface)
        .insert_price(
            "EQ-SPOT",
            MarketScalar::Price(Money::new(100.0, Currency::USD)),
        )
        .map_collateral("USD-CSA", CurveId::from("USD-OIS"));

    // Roundtrip via MarketContextState explicitly
    let state = MarketContextState::from(&ctx);
    let json = serde_json::to_string(&state).unwrap();
    let back_state: MarketContextState = serde_json::from_str(&json).unwrap();
    let rebuilt = MarketContext::try_from(back_state).unwrap();

    assert!(rebuilt.get_discount("USD-OIS").is_ok());
    assert!(rebuilt.get_forward("USD-SOFR").is_ok());
    assert!(rebuilt.get_hazard("CDX-HAZ").is_ok());
    assert!(rebuilt.get_hazard("ISSUER-HAZ").is_ok());
    assert!(rebuilt.get_inflation_curve("US-CPI-CURVE").is_ok());
    assert!(rebuilt.get_base_correlation("CDX-BC").is_ok());
    assert!(rebuilt.get_credit_index("CDX").is_ok());
    assert!(rebuilt.get_inflation_index("US-CPI").is_ok());
    assert!(rebuilt.get_series("TS").is_ok());
    assert!(rebuilt.get_surface("VOL").is_ok());
    assert!(rebuilt.get_price("EQ-SPOT").is_ok());
    assert!(rebuilt.get_collateral("USD-CSA").is_ok());
}

// =============================================================================
// Strict serde: unknown fields rejected on inbound curve/surface states
// =============================================================================

/// Asserts `value` round-trips through JSON and that adding one unknown
/// top-level field makes deserialization fail (deny_unknown_fields works).
fn assert_strict_inbound<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let json = serde_json::to_value(value).unwrap();

    // Original valid JSON still deserializes (wire format unchanged).
    let _: T = serde_json::from_value(json.clone()).unwrap();

    // Unknown top-level field is rejected.
    let mut tampered = json;
    tampered
        .as_object_mut()
        .expect("serialized state should be a JSON object")
        .insert("typo_field".to_string(), serde_json::json!(1));
    assert!(
        serde_json::from_value::<T>(tampered).is_err(),
        "unknown top-level field should be rejected"
    );
}

#[test]
fn discount_curve_rejects_unknown_fields() {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(test_date())
        .knots([(0.0, 1.0), (5.0, 0.9)])
        .build()
        .unwrap();
    assert_strict_inbound(&curve);
}

#[test]
fn forward_curve_rejects_unknown_fields() {
    let curve = ForwardCurve::builder("USD-SOFR", 0.25)
        .base_date(test_date())
        .knots([(0.0, 0.03), (5.0, 0.04)])
        .build()
        .unwrap();
    assert_strict_inbound(&curve);
}

#[test]
fn hazard_curve_rejects_unknown_fields() {
    let curve = HazardCurve::builder("CDX-HAZ")
        .base_date(test_date())
        .knots([(1.0, 0.01), (5.0, 0.02)])
        .build()
        .unwrap();
    assert_strict_inbound(&curve);
}

#[test]
fn inflation_curve_rejects_unknown_fields() {
    use finstack_core::market_data::term_structures::InflationCurve;
    let curve = InflationCurve::builder("US-CPI-CURVE")
        .base_cpi(100.0)
        .base_date(test_date())
        .knots([(0.0, 100.0), (1.0, 101.0)])
        .build()
        .unwrap();
    assert_strict_inbound(&curve);
}

#[test]
fn price_curve_rejects_unknown_fields() {
    use finstack_core::market_data::term_structures::PriceCurve;
    let curve = PriceCurve::builder("WTI")
        .base_date(test_date())
        .spot_price(70.0)
        .knots([(0.0, 70.0), (1.0, 72.0)])
        .build()
        .unwrap();
    assert_strict_inbound(&curve);
}

#[test]
fn volatility_index_curve_rejects_unknown_fields() {
    use finstack_core::market_data::term_structures::VolatilityIndexCurve;
    let curve = VolatilityIndexCurve::builder("VIX")
        .base_date(test_date())
        .spot_level(15.0)
        .knots([(0.0, 15.0), (1.0, 18.0)])
        .build()
        .unwrap();
    assert_strict_inbound(&curve);
}

#[test]
fn basis_spread_curve_rejects_unknown_fields() {
    use finstack_core::market_data::term_structures::BasisSpreadCurve;
    let curve = BasisSpreadCurve::builder("EURUSD-XCCY")
        .base_date(test_date())
        .knots([(0.0, -0.001), (5.0, -0.002)])
        .build()
        .unwrap();
    assert_strict_inbound(&curve);
}

#[test]
fn forward_variance_curve_rejects_unknown_fields() {
    use finstack_core::market_data::term_structures::ForwardVarianceCurve;
    let curve = ForwardVarianceCurve::from_points(&[(0.0, 0.04), (1.0, 0.05)]).unwrap();
    assert_strict_inbound(&curve);
}

#[test]
fn forward_variance_curve_rejects_invalid_values_on_deserialize() {
    use finstack_core::market_data::term_structures::ForwardVarianceCurve;
    // Structurally valid but semantically invalid: non-positive variance.
    let json = serde_json::json!({ "times": [0.0, 1.0], "values": [0.04, -0.05] });
    assert!(serde_json::from_value::<ForwardVarianceCurve>(json).is_err());
    // Mismatched lengths are also rejected.
    let json = serde_json::json!({ "times": [0.0, 1.0], "values": [0.04] });
    assert!(serde_json::from_value::<ForwardVarianceCurve>(json).is_err());
}

#[test]
fn diebold_li_rejects_unknown_fields() {
    use finstack_core::market_data::dtsm::DieboldLi;
    let model = DieboldLi::builder().build().unwrap();
    assert_strict_inbound(&model);
}

#[test]
fn diebold_li_rejects_invalid_lambda_on_deserialize() {
    use finstack_core::market_data::dtsm::DieboldLi;
    let model = DieboldLi::builder().build().unwrap();
    let mut json = serde_json::to_value(&model).unwrap();
    json.as_object_mut()
        .unwrap()
        .insert("lambda".to_string(), serde_json::json!(-1.0));
    assert!(serde_json::from_value::<DieboldLi>(json).is_err());
}

#[test]
fn fx_delta_vol_surface_rejects_unknown_fields() {
    use finstack_core::market_data::surfaces::FxDeltaVolSurface;
    let surface = FxDeltaVolSurface::new(
        "EURUSD-DELTA-VOL",
        vec![0.25, 0.5, 1.0],
        vec![0.08, 0.085, 0.09],
        vec![0.01, 0.012, 0.015],
        vec![0.005, 0.006, 0.007],
    )
    .unwrap();
    assert_strict_inbound(&surface);
}

#[test]
fn fx_delta_vol_surface_rejects_invalid_data_on_deserialize() {
    use finstack_core::market_data::surfaces::FxDeltaVolSurface;
    let surface = FxDeltaVolSurface::new(
        "EURUSD-DELTA-VOL",
        vec![0.25, 0.5, 1.0],
        vec![0.08, 0.085, 0.09],
        vec![0.01, 0.012, 0.015],
        vec![0.005, 0.006, 0.007],
    )
    .unwrap();
    let mut json = serde_json::to_value(&surface).unwrap();
    // Structurally valid but semantically invalid: non-monotonic expiries.
    json.as_object_mut()
        .unwrap()
        .insert("expiries".to_string(), serde_json::json!([1.0, 0.5, 2.0]));
    assert!(serde_json::from_value::<FxDeltaVolSurface>(json).is_err());
}
