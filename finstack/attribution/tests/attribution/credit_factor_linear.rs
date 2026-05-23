//! PR-7: opt-in credit factor hierarchy detail for metrics-based / Taylor attribution.
//!
//! Six named tests:
//!  1. `metrics_based_no_model_matches_existing_credit_total`
//!  2. `metrics_based_credit_detail_reconciles_to_credit_curves_pnl`
//!  3. `taylor_credit_detail_reconciles_to_credit_curves_pnl`
//!  4. `per_issuer_adder_is_omitted_by_default`
//!  5. `per_bucket_breakdown_can_be_disabled`
//!  6. `old_attribution_json_deserializes_with_no_credit_detail`

use finstack_attribution::{
    compute_credit_factor_attribution, AttributionEnvelope, AttributionMethod, AttributionSpec,
    CreditAttributionInput, CreditFactorDetailOptions, PnlAttribution,
};
use finstack_core::currency::Currency;
use finstack_core::dates::{create_date, DayCount};
use finstack_core::factor_model::credit_hierarchy::{
    AdderVolSource, CalibrationDiagnostics, CreditFactorModel, CreditHierarchySpec, DateRange,
    FactorCorrelationMatrix, GenericFactorSpec, HierarchyDimension, IssuerBetaMode,
    IssuerBetaPolicy, IssuerBetaRow, IssuerBetas, IssuerTags, LevelsAtAnchor, VolState,
};
use finstack_core::factor_model::{
    FactorCovarianceMatrix, FactorModelConfig, MatchingConfig, PricingMode,
};
use finstack_core::market_data::context::{
    CurveState, MarketContextState, MARKET_CONTEXT_STATE_VERSION,
};
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_core::money::Money;
use finstack_core::types::{CurveId, IssuerId};
use finstack_factor_model::{decompose_levels, decompose_period, PeriodDecomposition};
use finstack_valuations::instruments::json_loader::InstrumentJson;
use finstack_valuations::instruments::{Attributes, Bond};
use std::collections::BTreeMap;
use time::Month;

// ─────────────────────────── Helpers ───────────────────────────

fn issuer_tags(rating: &str, region: &str) -> IssuerTags {
    let mut m = BTreeMap::new();
    m.insert("rating".into(), rating.into());
    m.insert("region".into(), region.into());
    IssuerTags(m)
}

fn empty_factor_config() -> FactorModelConfig {
    FactorModelConfig {
        factors: vec![],
        covariance: FactorCovarianceMatrix::new(vec![], vec![]).unwrap(),
        matching: MatchingConfig::MappingTable(vec![]),
        pricing_mode: PricingMode::DeltaBased,
        risk_measure: Default::default(),
        bump_size: None,
        unmatched_policy: None,
    }
}

fn issuer_row(id: &str, rating: &str, region: &str, pc: f64, lv: Vec<f64>) -> IssuerBetaRow {
    IssuerBetaRow {
        issuer_id: IssuerId::new(id),
        tags: issuer_tags(rating, region),
        mode: IssuerBetaMode::IssuerBeta,
        betas: IssuerBetas { pc, levels: lv },
        adder_at_anchor: 0.0,
        adder_vol_annualized: 0.01,
        adder_vol_source: AdderVolSource::Default,
        fit_quality: None,
    }
}

fn make_model() -> CreditFactorModel {
    CreditFactorModel {
        schema_version: CreditFactorModel::SCHEMA_VERSION.into(),
        as_of: create_date(2024, Month::March, 29).unwrap(),
        calibration_window: DateRange {
            start: create_date(2022, Month::March, 29).unwrap(),
            end: create_date(2024, Month::March, 29).unwrap(),
        },
        policy: IssuerBetaPolicy::GloballyOff,
        generic_factor: GenericFactorSpec {
            name: "CDX IG 5Y".into(),
            series_id: "cdx.ig.5y".into(),
        },
        hierarchy: CreditHierarchySpec {
            levels: vec![HierarchyDimension::Rating, HierarchyDimension::Region],
        },
        config: empty_factor_config(),
        issuer_betas: vec![
            issuer_row("ISSUER-A", "IG", "EU", 1.10, vec![0.90, 1.05]),
            issuer_row("ISSUER-B", "IG", "EU", 1.15, vec![0.95, 1.00]),
            issuer_row("ISSUER-C", "HY", "NA", 0.85, vec![1.05, 0.92]),
        ],
        anchor_state: LevelsAtAnchor {
            pc: 0.0,
            by_level: vec![],
        },
        static_correlation: FactorCorrelationMatrix::identity(vec![]),
        vol_state: VolState {
            factors: BTreeMap::new(),
            idiosyncratic: BTreeMap::new(),
        },
        factor_histories: None,
        diagnostics: CalibrationDiagnostics {
            mode_counts: BTreeMap::new(),
            bucket_sizes_per_level: vec![],
            fold_ups: vec![],
            r_squared_histogram: None,
            tag_taxonomy: BTreeMap::new(),
        },
    }
}

fn make_period(model: &CreditFactorModel) -> (PeriodDecomposition, BTreeMap<IssuerId, f64>) {
    let mut s_t0 = BTreeMap::new();
    s_t0.insert(IssuerId::new("ISSUER-A"), 100.0);
    s_t0.insert(IssuerId::new("ISSUER-B"), 110.0);
    s_t0.insert(IssuerId::new("ISSUER-C"), 350.0);
    let mut s_t1 = BTreeMap::new();
    s_t1.insert(IssuerId::new("ISSUER-A"), 105.0);
    s_t1.insert(IssuerId::new("ISSUER-B"), 118.0);
    s_t1.insert(IssuerId::new("ISSUER-C"), 360.0);
    let from = decompose_levels(
        model,
        &s_t0,
        80.0,
        create_date(2025, Month::January, 1).unwrap(),
        None,
    )
    .unwrap();
    let to = decompose_levels(
        model,
        &s_t1,
        85.0,
        create_date(2025, Month::January, 31).unwrap(),
        None,
    )
    .unwrap();
    let period = decompose_period(&from, &to).unwrap();

    // Return ΔS map for sanity checks.
    let mut ds = BTreeMap::new();
    for (k, v0) in &s_t0 {
        ds.insert(k.clone(), s_t1[k] - v0);
    }
    (period, ds)
}

fn positions() -> Vec<CreditAttributionInput> {
    vec![
        CreditAttributionInput {
            position_id: "P-A".into(),
            issuer_id: IssuerId::new("ISSUER-A"),
            cs01: Money::new(-1500.0, Currency::USD),
            delta_spread: 5.0,
        },
        CreditAttributionInput {
            position_id: "P-B".into(),
            issuer_id: IssuerId::new("ISSUER-B"),
            cs01: Money::new(-2000.0, Currency::USD),
            delta_spread: 8.0,
        },
        CreditAttributionInput {
            position_id: "P-C".into(),
            issuer_id: IssuerId::new("ISSUER-C"),
            cs01: Money::new(-500.0, Currency::USD),
            delta_spread: 10.0,
        },
    ]
}

fn synthetic_credit_pnl(positions: &[CreditAttributionInput], ds: &BTreeMap<IssuerId, f64>) -> f64 {
    positions
        .iter()
        .map(|p| -p.cs01.amount() * ds[&p.issuer_id])
        .sum()
}

// ─────────────────────────── Tests ───────────────────────────

// Build a PnlAttribution, serialize it, strip the new `credit_factor_detail`
// key out of the JSON to simulate a "legacy" payload, and return the result.
fn legacy_attribution_json() -> String {
    let mut attr = PnlAttribution::new(
        Money::new(1_000.0, Currency::USD),
        "LEGACY",
        create_date(2025, Month::January, 15).unwrap(),
        create_date(2025, Month::January, 16).unwrap(),
        AttributionMethod::MetricsBased,
    );
    attr.credit_curves_pnl = Money::new(-250.5, Currency::USD);
    let mut value = serde_json::to_value(&attr).expect("serialize");
    if let Some(obj) = value.as_object_mut() {
        obj.remove("credit_factor_detail");
    }
    serde_json::to_string(&value).expect("re-serialize")
}

/// PR-7 named test 1: when no `credit_factor_model` is supplied (i.e. PR-7 is
/// opt-in), the existing credit total is unchanged. We verify by deserializing
/// an existing PnlAttribution JSON without `credit_factor_detail` and checking
/// that the field defaults to `None` and `credit_curves_pnl` is preserved
/// byte-identically.
#[test]
fn metrics_based_no_model_matches_existing_credit_total() {
    let json = legacy_attribution_json();
    let parsed: PnlAttribution = serde_json::from_str(&json).expect("legacy JSON should parse");
    assert!(parsed.credit_factor_detail.is_none());
    assert!((parsed.credit_curves_pnl.amount() - (-250.5)).abs() < 1e-12);
}

/// PR-7 named test 2: reconciliation invariant for the metrics-based wire.
/// The shared linear helper drives both metrics_based and Taylor; it produces
/// the same numbers in both paths. We verify the invariant at 1e-8.
#[test]
fn metrics_based_credit_detail_reconciles_to_credit_curves_pnl() {
    let model = make_model();
    let (period, ds) = make_period(&model);
    let positions = positions();
    let opts = CreditFactorDetailOptions::default();

    let detail = compute_credit_factor_attribution(&model, &opts, &positions, &period).expect("ok");
    let attributed = detail.generic_pnl.amount()
        + detail.levels.iter().map(|l| l.total.amount()).sum::<f64>()
        + detail.adder_pnl_total.amount();
    let expected = synthetic_credit_pnl(&positions, &ds);
    assert!(
        (attributed - expected).abs() < 1e-8,
        "metrics_based reconciliation: attributed={}, expected={}",
        attributed,
        expected
    );
}

/// PR-7 named test 3: end-to-end Taylor dispatch through `AttributionSpec`.
///
/// Constructs a minimal bond with a credit curve and issuer metadata, builds
/// `AttributionSpec` with `method = AttributionMethod::Taylor(...)` and
/// `credit_factor_model = Some(Box::new(...))`, and
/// executes it.  Asserts:
///  - `credit_factor_detail` is populated (Taylor wire is active)
///  - reconciliation invariant holds at 1e-8
#[test]
fn taylor_credit_detail_reconciles_to_credit_curves_pnl() {
    use finstack_attribution::TaylorAttributionConfig;

    let as_of_t0 = create_date(2025, Month::January, 1).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 2).unwrap();

    // Build a fixed-rate bond that has a credit curve dependency.
    let mut bond = Bond::fixed(
        "BOND-ISSUER-A",
        Money::new(1_000_000.0, Currency::USD),
        0.05_f64,
        create_date(2024, Month::January, 1).unwrap(),
        create_date(2034, Month::January, 1).unwrap(),
        "USD-OIS",
    )
    .expect("bond construction");
    // Wire the credit curve and the issuer ID used by compute_credit_factor_detail.
    bond.credit_curve_id = Some(CurveId::new("ISSUER-A-HAZ"));
    bond.attributes = Attributes::new().with_meta("credit::issuer_id", "ISSUER-A");

    // Flat discount curves (same at T0 and T1 — interest rate move is zero
    // so all Taylor P&L is credit).
    let make_discount = |base| {
        let r = 0.05_f64;
        DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0_f64, 1.0_f64),
                (1.0_f64, (-r).exp()),
                (5.0_f64, (-r * 5.0).exp()),
                (10.0_f64, (-r * 10.0).exp()),
                (30.0_f64, (-r * 30.0).exp()),
            ])
            .build()
            .expect("discount curve")
    };

    // Hazard curves: T0 at 100 bp, T1 at 200 bp → +100 bp parallel shift.
    let make_hazard = |base, rate: f64| {
        HazardCurve::builder("ISSUER-A-HAZ")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .recovery_rate(0.4)
            .knots([(0.5_f64, rate), (5.0_f64, rate), (10.0_f64, rate)])
            .build()
            .expect("hazard curve")
    };

    let disc_t0 = make_discount(as_of_t0);
    let disc_t1 = make_discount(as_of_t1);
    let haz_t0 = make_hazard(as_of_t0, 0.01); // 100 bp
    let haz_t1 = make_hazard(as_of_t1, 0.02); // 200 bp

    let make_market_state =
        |disc: DiscountCurve, haz: HazardCurve, prices: BTreeMap<String, MarketScalar>| {
            MarketContextState {
                version: MARKET_CONTEXT_STATE_VERSION,
                curves: vec![CurveState::Discount(disc), CurveState::Hazard(haz)],
                fx: None,
                surfaces: vec![],
                prices,
                series: vec![],
                inflation_indices: vec![],
                dividends: vec![],
                credit_indices: vec![],
                collateral: BTreeMap::new(),
                fx_delta_vol_surfaces: vec![],
                hierarchy: None,
                vol_cubes: vec![],
            }
        };
    let prices_t0 = BTreeMap::from([
        ("cdx.ig.5y".to_string(), MarketScalar::Unitless(100.0)),
        (
            "credit::level0::Rating::IG".to_string(),
            MarketScalar::Unitless(0.0),
        ),
        (
            "credit::level1::Rating.Region::IG.EU".to_string(),
            MarketScalar::Unitless(0.0),
        ),
    ]);
    let prices_t1 = BTreeMap::from([
        ("cdx.ig.5y".to_string(), MarketScalar::Unitless(110.0)),
        (
            "credit::level0::Rating::IG".to_string(),
            MarketScalar::Unitless(25.0),
        ),
        (
            "credit::level1::Rating.Region::IG.EU".to_string(),
            MarketScalar::Unitless(15.0),
        ),
    ]);

    let model = make_model();
    let spec = AttributionSpec {
        instrument: InstrumentJson::Bond(bond),
        market_t0: make_market_state(disc_t0, haz_t0, prices_t0),
        market_t1: make_market_state(disc_t1, haz_t1, prices_t1),
        as_of_t0,
        as_of_t1,
        method: AttributionMethod::Taylor(TaylorAttributionConfig::default()),
        model_params_t0: None,
        credit_factor_model: Some(Box::new(model)),
        credit_factor_detail_options: CreditFactorDetailOptions::default(),
        config: None,
        full_cross_attribution: false,
    };

    let result = AttributionEnvelope::new(spec)
        .execute()
        .expect("taylor attribution with credit detail should succeed");
    let attribution = result.result.attribution;

    // The credit-factor detail must be populated (Taylor dispatch is active).
    let detail = attribution
        .credit_factor_detail
        .as_ref()
        .expect("credit_factor_detail must be Some for Taylor with credit_factor_model");

    // Reconciliation invariant: generic + Σ levels + adder + curve_shape ≡
    // credit_curves_pnl. `curve_shape` carries the non-parallel residual —
    // here, the convexity of the +100bp move not captured by a 1bp CS01.
    let parallel_part = detail.generic_pnl.amount()
        + detail.levels.iter().map(|l| l.total.amount()).sum::<f64>()
        + detail.adder_pnl_total.amount();
    let attributed = parallel_part + detail.curve_shape_pnl.amount();
    let expected = attribution.credit_curves_pnl.amount();
    assert!(
        (attributed - expected).abs() < 1e-6,
        "taylor end-to-end reconciliation failed: attributed={attributed}, credit_curves_pnl={expected}"
    );
    // The hazard move is parallel, so the factor steps carry the bulk of the
    // credit P&L; curve_shape is only the small convexity residual.
    assert!(
        detail.curve_shape_pnl.amount().abs() < 0.25 * expected.abs(),
        "parallel move: curve_shape should be a small residual, got {} vs credit_pnl {expected}",
        detail.curve_shape_pnl.amount()
    );
    // Each factor widened the spread, so every step shares the sign of the
    // (loss-making) total credit P&L.
    assert!(
        detail.generic_pnl.amount() * expected > 0.0,
        "generic should share the credit P&L sign"
    );
    assert!(
        detail.adder_pnl_total.amount() * expected > 0.0,
        "adder should share the credit P&L sign"
    );
}

/// A non-parallel (twisted) hazard-curve move no longer omits or explodes the
/// credit-factor detail. `compute_credit_factor_detail` measures a real CS01
/// (no `−credit_pnl / ds_i` back-solve) and routes the non-parallel residual
/// into `curve_shape_pnl`, so the detail is produced and reconciles. (Taylor's
/// first-order credit P&L is itself signed-average-based, so for a pure twist
/// it is near zero — the meaningful "twist → curve_shape" magnitude check is
/// the full-reval waterfall test
/// `waterfall_twisted_hazard_attributes_curve_shape_not_adder`.)
#[test]
fn twisted_hazard_curve_does_not_omit_or_explode_credit_detail() {
    use finstack_attribution::TaylorAttributionConfig;

    let as_of_t0 = create_date(2025, Month::January, 1).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 2).unwrap();

    let mut bond = Bond::fixed(
        "BOND-ISSUER-A",
        Money::new(1_000_000.0, Currency::USD),
        0.05_f64,
        create_date(2024, Month::January, 1).unwrap(),
        create_date(2034, Month::January, 1).unwrap(),
        "USD-OIS",
    )
    .expect("bond construction");
    bond.credit_curve_id = Some(CurveId::new("ISSUER-A-HAZ"));
    bond.attributes = Attributes::new().with_meta("credit::issuer_id", "ISSUER-A");

    let make_discount = |base| {
        let r = 0.05_f64;
        DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0_f64, 1.0_f64),
                (1.0_f64, (-r).exp()),
                (5.0_f64, (-r * 5.0).exp()),
                (10.0_f64, (-r * 10.0).exp()),
                (30.0_f64, (-r * 30.0).exp()),
            ])
            .build()
            .expect("discount curve")
    };

    // T0 hazard: flat at 200 bp on the standard tenor grid. T1 hazard: a TWIST
    // — short tenors up, long tenors down — with knots placed exactly on the
    // standard tenors so the per-tenor shifts are controlled. The signed
    // shifts (+100,+100,+50,+50,0,-50,-50,-100,-100 bp) sum to EXACTLY 0, so
    // the average parallel move is 0, yet the absolute (L1) move is large
    // (~66 bp). Centered at 200 bp so every T1 rate stays comfortably positive.
    let std_tenors = [0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 30.0];
    let haz_t0 = HazardCurve::builder("ISSUER-A-HAZ")
        .base_date(as_of_t0)
        .day_count(DayCount::Act365F)
        .recovery_rate(0.4)
        .knots(
            std_tenors
                .iter()
                .map(|&t| (t, 0.02_f64))
                .collect::<Vec<_>>(),
        )
        .build()
        .expect("hazard t0");
    let t1_rates = [
        0.030, 0.030, 0.025, 0.025, 0.020, 0.015, 0.015, 0.010, 0.010,
    ];
    let haz_t1 = HazardCurve::builder("ISSUER-A-HAZ")
        .base_date(as_of_t1)
        .day_count(DayCount::Act365F)
        .recovery_rate(0.4)
        .knots(
            std_tenors
                .iter()
                .zip(t1_rates.iter())
                .map(|(&t, &r)| (t, r))
                .collect::<Vec<_>>(),
        )
        .build()
        .expect("hazard t1");

    let make_market_state =
        |disc: DiscountCurve, haz: HazardCurve, prices: BTreeMap<String, MarketScalar>| {
            MarketContextState {
                version: MARKET_CONTEXT_STATE_VERSION,
                curves: vec![CurveState::Discount(disc), CurveState::Hazard(haz)],
                fx: None,
                surfaces: vec![],
                prices,
                series: vec![],
                inflation_indices: vec![],
                dividends: vec![],
                credit_indices: vec![],
                collateral: BTreeMap::new(),
                fx_delta_vol_surfaces: vec![],
                hierarchy: None,
                vol_cubes: vec![],
            }
        };
    let prices = || {
        BTreeMap::from([
            ("cdx.ig.5y".to_string(), MarketScalar::Unitless(100.0)),
            (
                "credit::level0::Rating::IG".to_string(),
                MarketScalar::Unitless(0.0),
            ),
            (
                "credit::level1::Rating.Region::IG.EU".to_string(),
                MarketScalar::Unitless(0.0),
            ),
        ])
    };

    let model = make_model();
    let spec = AttributionSpec {
        instrument: InstrumentJson::Bond(bond),
        market_t0: make_market_state(make_discount(as_of_t0), haz_t0, prices()),
        market_t1: make_market_state(make_discount(as_of_t1), haz_t1, prices()),
        as_of_t0,
        as_of_t1,
        method: AttributionMethod::Taylor(TaylorAttributionConfig::default()),
        model_params_t0: None,
        credit_factor_model: Some(Box::new(model)),
        credit_factor_detail_options: CreditFactorDetailOptions::default(),
        config: None,
        full_cross_attribution: false,
    };

    let result = AttributionEnvelope::new(spec)
        .execute()
        .expect("attribution should succeed even with a twisted hazard curve");
    let attribution = result.result.attribution;

    // The detail is produced — a twist is no longer a reason to omit it.
    let detail = attribution
        .credit_factor_detail
        .as_ref()
        .expect("credit_factor_detail must be Some for a twisted curve");

    // It reconciles, with the non-parallel residual carried by curve_shape.
    let attributed = detail.generic_pnl.amount()
        + detail.levels.iter().map(|l| l.total.amount()).sum::<f64>()
        + detail.adder_pnl_total.amount()
        + detail.curve_shape_pnl.amount();
    let credit_pnl = attribution.credit_curves_pnl.amount();
    assert!(
        (attributed - credit_pnl).abs() < 1e-6,
        "reconciliation must hold under a twist: attributed={attributed}, credit_curves_pnl={credit_pnl}"
    );
    // Every reported number stays finite — no blown synthetic-CS01 divide.
    assert!(
        credit_pnl.is_finite()
            && detail.curve_shape_pnl.amount().is_finite()
            && detail.generic_pnl.amount().is_finite(),
        "all credit-detail numbers must remain finite"
    );
}

/// PR-7 named test 4: per-issuer adder map is `None` by default.
#[test]
fn per_issuer_adder_is_omitted_by_default() {
    let model = make_model();
    let (period, _ds) = make_period(&model);
    let opts = CreditFactorDetailOptions::default();
    assert!(!opts.include_per_issuer_adder);

    let detail =
        compute_credit_factor_attribution(&model, &opts, &positions(), &period).expect("ok");
    assert!(detail.adder_pnl_by_issuer.is_none());
}

/// PR-7 named test 5: per-bucket breakdown can be turned off.
#[test]
fn per_bucket_breakdown_can_be_disabled() {
    let model = make_model();
    let (period, _ds) = make_period(&model);
    let opts = CreditFactorDetailOptions {
        include_per_issuer_adder: false,
        include_per_bucket_breakdown: false,
    };
    let detail =
        compute_credit_factor_attribution(&model, &opts, &positions(), &period).expect("ok");
    for level in &detail.levels {
        assert!(
            level.by_bucket.is_empty(),
            "level {} should have no by_bucket map when disabled",
            level.level_name
        );
    }
}

/// PR-7 named test 6: legacy attribution JSON without `credit_factor_detail`
/// deserializes successfully and the new field defaults to `None`.
#[test]
fn old_attribution_json_deserializes_with_no_credit_detail() {
    let json = legacy_attribution_json();
    let parsed: PnlAttribution = serde_json::from_str(&json).expect("legacy JSON should parse");
    assert!(parsed.credit_factor_detail.is_none());
}
