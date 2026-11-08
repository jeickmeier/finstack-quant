//! Private I1 owning-map facts; this module is included only by the context owner.
//!
//! Fixtures mirror the frozen core B3 benchmark/public-panel inputs. Full numerical,
//! Decimal and ordered-error records remain in `context-bump-reference.json`.
//! This diagnostic captures exactly one public bump per case and no timing.
//!
//! After root archives C0, apply this overlay and run (fresh absolute directory):
//! `mise run rust-test-filter -- finstack-quant-core context_bump_work_matches_frozen_pattern --lib`
//! `CCM_CORE_WORK_EXPORT_DIR=/absolute/new/C0/core mise run rust-test-filter -- finstack-quant-core export_context_bump_work_reference --lib --slow`
//!
//! C0 uses the constant below. At the authorized presence-guard stage, switch only
//! that selector to `Candidate`, retaining both frozen matrices. Move the hook's
//! observation inside the same presence condition as the actual generic make_mut.
//! Never infer the expected revision from observed events or change the numeric panel.

use super::{MarketContext, MarketContextState};
use crate::currency::Currency;
use crate::dates::Date;
use crate::market_data::bumps::{BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump};
use crate::market_data::scalars::{MarketScalar, ScalarTimeSeries, SeriesInterpolation};
use crate::market_data::surfaces::VolSurface;
use crate::market_data::term_structures::{
    BaseCorrelationCurve, CreditIndexData, DiscountCurve, HazardCurve,
};
use crate::money::Money;
use crate::types::CurveId;
use crate::{Error, InputError};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::Write as _;
use std::sync::Arc;
use time::{Duration, Month};

#[derive(Clone, Copy)]
enum WorkPattern {
    C0,
    Candidate,
}

// Diagnostic expectation only; the production algorithm is unchanged by this overlay.
const EXPECTED_PATTERN: WorkPattern = WorkPattern::Candidate;

impl WorkPattern {
    fn name(self) -> &'static str {
        match self {
            Self::C0 => "c0",
            Self::Candidate => "candidate",
        }
    }
}

#[derive(Debug, Serialize)]
struct CurveMapEvent {
    target_id: String,
    map_len_before: usize,
    strong_count_before: usize,
    weak_count_before: usize,
}

thread_local! {
    static CURVE_EVENTS: RefCell<Option<Vec<CurveMapEvent>>> = const { RefCell::new(None) };
}

// Called immediately before the actual generic curves-map Arc::make_mut. No Arc
// is cloned to observe it; unselected tests record nothing and allocate no events.
pub(super) fn record_generic_curves_make_mut(context: &MarketContext, target_id: &str) {
    CURVE_EVENTS.with(|slot| {
        if let Some(events) = slot.borrow_mut().as_mut() {
            events.push(CurveMapEvent {
                target_id: target_id.to_owned(),
                map_len_before: context.curves.len(),
                strong_count_before: Arc::strong_count(&context.curves),
                weak_count_before: Arc::weak_count(&context.curves),
            });
        }
    });
}

struct CaptureScope;

impl CaptureScope {
    fn start() -> Self {
        CURVE_EVENTS.with(|slot| {
            let mut slot = slot.borrow_mut();
            assert!(slot.is_none(), "nested I1 work capture");
            *slot = Some(Vec::new());
        });
        Self
    }

    fn finish(self) -> Vec<CurveMapEvent> {
        let events =
            CURVE_EVENTS.with(|slot| slot.borrow_mut().take().expect("active I1 work capture"));
        drop(self);
        events
    }
}

impl Drop for CaptureScope {
    fn drop(&mut self) {
        // Also reset during unwinding, without retaining another context or Arc.
        CURVE_EVENTS.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}

struct WorkCase {
    id: String,
    base: MarketContext,
    expected_curve_count: usize,
    bumps: Vec<MarketBump>,
    changed_stores: &'static [&'static str],
    c0_targets: &'static [&'static str],
    candidate_targets: &'static [&'static str],
    expected_missing_id: Option<&'static str>,
}

fn date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).expect("frozen B3 date")
}

fn curve(id: &str) -> DiscountCurve {
    DiscountCurve::builder(id)
        .base_date(date())
        .knots((0..40).map(|i| {
            let time = f64::from(i) * 0.5;
            (time, (-0.04 * time).exp())
        }))
        .build()
        .expect("frozen B3 curve")
}

fn curve_fixture(count: usize) -> MarketContext {
    let mut market = MarketContext::new();
    for index in 0..count {
        market.insert_mut(curve(&format!("CURVE_{index}")));
    }
    market
}

fn mixed_fixture(count: usize) -> MarketContext {
    // Exact bounded stores from core/benches/context_bump.rs and the public panel.
    let surface = VolSurface::from_grid(
        "EQ-VOL",
        &[0.25, 1.0, 2.0],
        &[80.0, 100.0, 120.0],
        &[0.18, 0.19, 0.20, 0.20, 0.21, 0.22, 0.22, 0.23, 0.24],
        Default::default(),
        None,
    )
    .expect("frozen B3 surface");
    let series = ScalarTimeSeries::new(
        "SPX-CLOSE",
        (0..256)
            .map(|i| (date() + Duration::days(i), 100.0 + 0.1 * i as f64))
            .collect(),
        Some(Currency::USD),
    )
    .expect("frozen B3 series")
    .with_interpolation(SeriesInterpolation::Step);
    curve_fixture(count)
        .insert_surface(surface)
        .insert_price(
            "SPX-SPOT",
            MarketScalar::Price(Money::from((100_i64, Currency::USD))),
        )
        .insert_series(series)
}

fn credit_fixture() -> MarketContext {
    let hazard = HazardCurve::flat("CDX-HZD", date(), 0.02, 0.4).expect("index hazard");
    let issuer = HazardCurve::flat("ISSUER-HZD", date(), 0.03, 0.4).expect("issuer hazard");
    let correlation = BaseCorrelationCurve::builder("CDX-BC")
        .knots([(3.0, 0.2), (7.0, 0.4), (10.0, 0.5)])
        .build()
        .expect("base correlation");
    let index = CreditIndexData::builder()
        .num_constituents(1)
        .recovery_rate(0.4)
        .index_credit_curve(Arc::new(hazard.clone()))
        .base_correlation_curve(Arc::new(correlation.clone()))
        .issuer_curves([("Issuer".to_string(), Arc::new(issuer.clone()))])
        .build()
        .expect("credit index");
    mixed_fixture(8)
        .insert(hazard)
        .insert(issuer)
        .insert(correlation)
        .insert_credit_index("CDX", index)
        .expect("canonical credit index")
}

fn generic(id: &str, spec: BumpSpec) -> MarketBump {
    MarketBump::Curve {
        id: CurveId::new(id),
        spec,
    }
}

fn additive(value: f64) -> BumpSpec {
    BumpSpec {
        mode: BumpMode::Additive,
        units: BumpUnits::Fraction,
        value,
        bump_type: BumpType::Parallel,
    }
}

fn cases() -> Vec<WorkCase> {
    let mut cases = Vec::with_capacity(13);
    for count in [8, 64, 256, 1024] {
        cases.push(WorkCase {
            id: format!("context_bump_mixed/money_multiplier/curves_{count}"),
            base: mixed_fixture(count),
            expected_curve_count: count,
            bumps: vec![generic("SPX-SPOT", BumpSpec::multiplier(1.01))],
            changed_stores: &["prices"],
            c0_targets: &["SPX-SPOT"],
            candidate_targets: &[],
            expected_missing_id: None,
        });
    }
    cases.extend([
        WorkCase {
            id: "context_bump_mixed/surface_abs/curves_256".into(),
            base: mixed_fixture(256),
            expected_curve_count: 256,
            bumps: vec![generic("EQ-VOL", additive(0.01))],
            changed_stores: &["surfaces"],
            c0_targets: &["EQ-VOL"],
            candidate_targets: &[],
            expected_missing_id: None,
        },
        WorkCase {
            id: "context_bump_mixed/series_multiplier/curves_256".into(),
            base: mixed_fixture(256),
            expected_curve_count: 256,
            bumps: vec![generic("SPX-CLOSE", BumpSpec::multiplier(1.01))],
            changed_stores: &["series"],
            c0_targets: &["SPX-CLOSE"],
            candidate_targets: &[],
            expected_missing_id: None,
        },
        WorkCase {
            id: "context_bump_mixed/typed_vol_bucket/curves_256".into(),
            base: mixed_fixture(256),
            expected_curve_count: 256,
            bumps: vec![MarketBump::VolBucketPct {
                vol_surface_id: CurveId::new("EQ-VOL"),
                expiries: None,
                strikes: None,
                pct: 5.0,
            }],
            changed_stores: &["surfaces"],
            c0_targets: &[],
            candidate_targets: &[],
            expected_missing_id: None,
        },
        WorkCase {
            id: "context_bump/256_curves".into(),
            base: curve_fixture(256),
            expected_curve_count: 256,
            bumps: vec![generic("CURVE_0", BumpSpec::parallel_bp(1.0))],
            changed_stores: &["curves"],
            c0_targets: &["CURVE_0"],
            candidate_targets: &["CURVE_0"],
            expected_missing_id: None,
        },
        WorkCase {
            id: "semantic/order/interleaved_repeat".into(),
            base: mixed_fixture(8),
            expected_curve_count: 8,
            bumps: vec![
                generic("SPX-SPOT", additive(10.0)),
                generic("EQ-VOL", additive(0.01)),
                generic("SPX-CLOSE", BumpSpec::multiplier(1.01)),
                generic("SPX-SPOT", BumpSpec::multiplier(1.1)),
            ],
            changed_stores: &["prices", "surfaces", "series"],
            c0_targets: &["SPX-SPOT", "EQ-VOL", "SPX-CLOSE", "SPX-SPOT"],
            candidate_targets: &[],
            expected_missing_id: None,
        },
        WorkCase {
            id: "semantic/order/repeated_actual_curve".into(),
            base: mixed_fixture(8),
            expected_curve_count: 8,
            bumps: vec![
                generic("CURVE_0", BumpSpec::parallel_bp(1.0)),
                generic("CURVE_0", BumpSpec::parallel_bp(2.0)),
            ],
            changed_stores: &["curves"],
            c0_targets: &["CURVE_0", "CURVE_0"],
            candidate_targets: &["CURVE_0", "CURVE_0"],
            expected_missing_id: None,
        },
        WorkCase {
            id: "semantic/credit/ordered_dependency_rebind".into(),
            base: credit_fixture(),
            expected_curve_count: 11,
            bumps: vec![
                generic("CDX-HZD", BumpSpec::parallel_bp(6.0)),
                generic("ISSUER-HZD", BumpSpec::parallel_bp(12.0)),
                generic("CDX-BC", additive(0.01)),
                generic("CDX-HZD", BumpSpec::parallel_bp(3.0)),
            ],
            changed_stores: &["curves", "credit_indices"],
            c0_targets: &["CDX-HZD", "ISSUER-HZD", "CDX-BC", "CDX-HZD"],
            candidate_targets: &["CDX-HZD", "ISSUER-HZD", "CDX-BC", "CDX-HZD"],
            expected_missing_id: None,
        },
        WorkCase {
            id: "semantic/empty_bumps".into(),
            base: mixed_fixture(8),
            expected_curve_count: 8,
            bumps: Vec::new(),
            changed_stores: &[],
            c0_targets: &[],
            candidate_targets: &[],
            expected_missing_id: None,
        },
        WorkCase {
            id: "semantic/errors/first_missing_ZZ-MISSING".into(),
            base: mixed_fixture(8),
            expected_curve_count: 8,
            bumps: vec![
                generic("SPX-SPOT", BumpSpec::multiplier(1.01)),
                generic("ZZ-MISSING", additive(0.01)),
                generic("AA-MISSING", additive(0.01)),
            ],
            changed_stores: &[],
            c0_targets: &["SPX-SPOT", "ZZ-MISSING"],
            candidate_targets: &[],
            expected_missing_id: Some("ZZ-MISSING"),
        },
    ]);
    assert_eq!(cases.len(), 13);
    let ids: BTreeSet<_> = cases.iter().map(|case| case.id.as_str()).collect();
    assert_eq!(ids.len(), 13, "unique frozen work case IDs");
    cases
}

fn store_sharing(base: &MarketContext, bumped: &MarketContext) -> BTreeMap<&'static str, bool> {
    BTreeMap::from([
        ("curves", Arc::ptr_eq(&base.curves, &bumped.curves)),
        ("surfaces", Arc::ptr_eq(&base.surfaces, &bumped.surfaces)),
        ("prices", Arc::ptr_eq(&base.prices, &bumped.prices)),
        ("series", Arc::ptr_eq(&base.series, &bumped.series)),
        (
            "credit_indices",
            Arc::ptr_eq(&base.credit_indices, &bumped.credit_indices),
        ),
        (
            "inflation_indices",
            Arc::ptr_eq(&base.inflation_indices, &bumped.inflation_indices),
        ),
        ("dividends", Arc::ptr_eq(&base.dividends, &bumped.dividends)),
        (
            "fx_delta_vol_surfaces",
            Arc::ptr_eq(&base.fx_delta_vol_surfaces, &bumped.fx_delta_vol_surfaces),
        ),
        ("vol_cubes", Arc::ptr_eq(&base.vol_cubes, &bumped.vol_cubes)),
    ])
}

fn expected_targets(case: &WorkCase, pattern: WorkPattern) -> &'static [&'static str] {
    match pattern {
        WorkPattern::C0 => case.c0_targets,
        WorkPattern::Candidate => case.candidate_targets,
    }
}

fn expected_sharing(case: &WorkCase, pattern: WorkPattern) -> Option<BTreeMap<&'static str, bool>> {
    if case.expected_missing_id.is_some() {
        return None;
    }
    let mut shared = BTreeMap::from([
        ("curves", true),
        ("surfaces", true),
        ("prices", true),
        ("series", true),
        ("credit_indices", true),
        ("inflation_indices", true),
        ("dividends", true),
        ("fx_delta_vol_surfaces", true),
        ("vol_cubes", true),
    ]);
    for &store in case.changed_stores {
        *shared.get_mut(store).expect("known changed owning store") = false;
    }
    if matches!(pattern, WorkPattern::C0) && !case.c0_targets.is_empty() {
        shared.insert("curves", false);
    }
    Some(shared)
}

fn expectation(case: &WorkCase, pattern: WorkPattern) -> Value {
    let targets = expected_targets(case, pattern);
    json!({
        "case_id": case.id,
        "generic_curves_map_make_mut_calls": targets.len(),
        "generic_curves_map_make_mut_shared_calls": usize::from(!targets.is_empty()),
        "event_target_ids": targets,
        "event_shared_before": (0..targets.len()).map(|index| index == 0).collect::<Vec<_>>(),
        "map_len_before": case.expected_curve_count,
        "owning_maps_shared": expected_sharing(case, pattern),
        "missing_id": case.expected_missing_id,
    })
}

fn state(market: &MarketContext) -> Value {
    serde_json::to_value(MarketContextState::try_from(market).expect("coherent fixture state"))
        .expect("serialize state outside work capture")
}

fn bits(value: f64) -> String {
    format!("{:016x}", value.to_bits())
}

fn ordered_bumps(bumps: &[MarketBump]) -> Vec<Value> {
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
        MarketBump::FxPct { base, quote, pct, as_of } => json!({
            "index": index, "kind": "fx_percentage", "base": base.to_string(),
            "quote": quote.to_string(), "pct_bits": bits(*pct), "as_of": as_of.to_string(),
        }),
    }).collect()
}

fn record(case: &WorkCase) -> Value {
    assert_eq!(
        case.base.curves.len(),
        case.expected_curve_count,
        "{}",
        case.id
    );
    let before = state(&case.base);
    let inputs = ordered_bumps(&case.bumps);
    let operation_bumps = case.bumps.clone();
    let scope = CaptureScope::start();
    let result = case.base.bump(operation_bumps);
    let events = scope.finish();
    // Everything below, including snapshots and owning-map observations, is
    // outside capture; no state serialization can inflate the event counts.
    assert!(CURVE_EVENTS.with(|slot| slot.borrow().is_none()));
    assert_eq!(state(&case.base), before, "source mutation in {}", case.id);
    let targets = expected_targets(case, EXPECTED_PATTERN);
    assert_eq!(events.len(), targets.len(), "{}", case.id);
    for (index, (event, expected_target)) in events.iter().zip(targets).enumerate() {
        assert_eq!(event.target_id, *expected_target, "{}", case.id);
        assert_eq!(
            event.map_len_before, case.expected_curve_count,
            "{}",
            case.id
        );
        assert_eq!(event.strong_count_before > 1, index == 0, "{}", case.id);
        assert_eq!(event.weak_count_before, 0, "{}", case.id);
    }
    let shared_calls = events
        .iter()
        .filter(|event| event.strong_count_before > 1)
        .count();
    assert_eq!(
        shared_calls,
        usize::from(!targets.is_empty()),
        "{}",
        case.id
    );
    let (sharing, error) = match result {
        Ok(bumped) => {
            assert!(case.expected_missing_id.is_none(), "{}", case.id);
            let actual = store_sharing(&case.base, &bumped);
            assert_eq!(
                Some(&actual),
                expected_sharing(case, EXPECTED_PATTERN).as_ref(),
                "{}",
                case.id
            );
            if case.bumps.is_empty() {
                assert_eq!(state(&bumped), before);
            } else {
                assert_ne!(state(&bumped), before, "expected a successful state change");
            }
            (Some(actual), None)
        }
        Err(error) => {
            let expected = case.expected_missing_id.expect("unexpected bump failure");
            let Error::Input(InputError::NotFound { id }) = &error else {
                panic!("{}: unexpected error {error:?}", case.id);
            };
            assert_eq!(id, expected, "{}", case.id);
            (
                None,
                Some(json!({"variant": "Input::NotFound", "id": id, "message": error.to_string()})),
            )
        }
    };
    json!({
        "case_id": case.id, "selected_public_bump_operations": 1,
        "curve_count": case.base.curves.len(), "unrelated_discount_curve_knots": 40,
        "surface_count": case.base.surfaces.len(), "price_count": case.base.prices.len(),
        "series_count": case.base.series.len(), "credit_index_count": case.base.credit_indices.len(),
        "ordered_bumps": inputs, "source_state_unchanged": true,
        "generic_curves_map_make_mut_calls": events.len(),
        "generic_curves_map_make_mut_shared_calls": shared_calls,
        "events": events, "owning_maps_shared": sharing, "error": error,
    })
}

fn sha256(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut encoded, "{byte:02x}").expect("format SHA-256 byte");
    }
    encoded
}

fn panel() -> Value {
    let cases = cases();
    let c0 = cases
        .iter()
        .map(|case| expectation(case, WorkPattern::C0))
        .collect::<Vec<_>>();
    let candidate = cases
        .iter()
        .map(|case| expectation(case, WorkPattern::Candidate))
        .collect::<Vec<_>>();
    let records = cases.iter().map(record).collect::<Vec<_>>();
    assert_eq!(records.len(), 13);
    json!({
        "schema": "finstack_quant.ccm_context_bump_work/1",
        "expected_pattern": EXPECTED_PATTERN.name(),
        "selected_public_bump_operations": 13,
        "counter_scope": "actual generic apply_curve_bump curves Arc::make_mut calls only; inactive outside one scoped bump per case",
        "diagnostic_source_sha256": sha256(include_bytes!("context_bump_work.rs")),
        "hook_source_sha256": sha256(include_bytes!("../../src/market_data/context/ops_bump.rs")),
        "fixture_sources": ["core/benches/context_bump.rs", "core/tests/performance/context_bump.rs"],
        "frozen_expectations": {"c0": c0, "candidate": candidate},
        "records": records,
        "allocations": null, "copied_bytes": null,
        "limitations": "Map length is a fixture dimension, not a count of cloned entries, bytes or allocations. Per-child-curve sharing is not owning-map sharing. Typed base-correlation mutation is outside this generic hook. Numerical/Decimal records remain in the separate public panel.",
    })
}

#[test]
fn context_bump_work_matches_frozen_pattern() {
    let _ = panel();
}

#[test]
#[ignore = "exports private I1 work facts separately from numerical C0 records"]
fn export_context_bump_work_reference() {
    let directory =
        std::env::var_os("CCM_CORE_WORK_EXPORT_DIR").expect("CCM_CORE_WORK_EXPORT_DIR is required");
    let directory = std::path::PathBuf::from(directory);
    let payload = panel();
    std::fs::create_dir_all(&directory).expect("create work export directory");
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("context-bump-work.json"))
        .expect("create new immutable work reference");
    output
        .write_all(&serde_json::to_vec_pretty(&payload).expect("work JSON"))
        .expect("write work reference");
}
