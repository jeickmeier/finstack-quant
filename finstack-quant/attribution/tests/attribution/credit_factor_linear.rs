//! Credit factor hierarchy detail for metrics-based and Taylor attribution.
//!
//! Three focused tests:
//!  1. `metrics_based_no_model_matches_existing_credit_total`
//!  2. `taylor_credit_detail_reconciles_to_credit_curves_pnl`
//!  3. `twisted_hazard_curve_does_not_omit_or_explode_credit_detail`

use crate::attribution_support::calibrated_hazard_curve;
use finstack_quant_attribution::{
    AttributionEnvelope, AttributionMethod, AttributionSpec, CreditFactorDetailOptions,
    PnlAttribution,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{create_date, DayCount};
use finstack_quant_core::market_data::context::{CurveState, MarketContextState};
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, IssuerId};
use finstack_quant_models::factor::credit::hierarchy::{
    AdderVolSource, CalibrationDiagnostics, CreditFactorModel, CreditHierarchySpec, DateRange,
    FactorCorrelationMatrix, GenericFactorSpec, HierarchyDimension, IssuerBetaMode,
    IssuerBetaPolicy, IssuerBetaRow, IssuerBetas, IssuerTags, LevelsAtAnchor, VolState,
};
use finstack_quant_models::factor::{
    FactorCovarianceMatrix, FactorModelConfig, MatchingConfig, PricingMode,
};
use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
use finstack_quant_valuations::instruments::{Attributes, Bond};
use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
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
        level_fit_quality: vec![],
        spread_duration: 1.0,
    }
}

fn make_model() -> CreditFactorModel {
    CreditFactorModel {
        schema: finstack_quant_models::factor::credit::hierarchy::CreditFactorModelSchema::CURRENT,
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
        panel_frequency:
            finstack_quant_models::factor::credit::calibration::PanelFrequency::Monthly,
        use_returns_or_levels:
            finstack_quant_models::factor::credit::calibration::PanelSpace::Returns,
        bucket_weighting:
            finstack_quant_models::factor::credit::calibration::BucketWeighting::Equal,
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

// ─────────────────────────── Tests ───────────────────────────

/// When no credit factor model is supplied, the canonical optional detail is
/// absent and the aggregate credit P&L survives a wire round trip unchanged.
#[test]
fn metrics_based_no_model_matches_existing_credit_total() {
    let mut attribution = PnlAttribution::new(
        Money::new(1_000.0, Currency::USD).expect("valid money fixture"),
        "NO-MODEL",
        create_date(2025, Month::January, 15).unwrap(),
        create_date(2025, Month::January, 16).unwrap(),
        AttributionMethod::MetricsBased,
    );
    attribution.credit_curves_pnl = Money::new(-250.5, Currency::USD).expect("valid money fixture");
    let json = serde_json::to_string(&attribution).expect("serialize attribution");
    let parsed: PnlAttribution = serde_json::from_str(&json).expect("deserialize attribution");
    assert!(parsed.credit_factor_detail.is_none());
    assert!((parsed.credit_curves_pnl.amount() - (-250.5)).abs() < 1e-12);
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
    use finstack_quant_attribution::TaylorAttributionConfig;

    let as_of_t0 = create_date(2025, Month::January, 1).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 2).unwrap();

    // Build a fixed-rate bond that has a credit curve dependency.
    let mut bond = Bond::fixed(
        "BOND-ISSUER-A",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05_f64).expect("valid rate fixture"),
        create_date(2024, Month::January, 1).unwrap(),
        create_date(2034, Month::January, 1).unwrap(),
        finstack_quant_core::dates::StubKind::ShortFront,
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

    let disc_t0 = make_discount(as_of_t0);
    let disc_t1 = make_discount(as_of_t1);
    let convention = CdsConventionKey {
        currency: Currency::USD,
        doc_clause: CdsDocClause::IsdaNa,
    };
    let haz_t0 = calibrated_hazard_curve(
        &disc_t0,
        as_of_t0,
        "ISSUER-A-HAZ",
        "ISSUER-A",
        0.4,
        convention.clone(),
        &[(1, 60.0), (3, 60.0), (5, 60.0), (10, 60.0)],
    )
    .expect("T0 hazard calibration");
    let haz_t1 = calibrated_hazard_curve(
        &disc_t1,
        as_of_t1,
        "ISSUER-A-HAZ",
        "ISSUER-A",
        0.4,
        convention,
        &[(1, 120.0), (3, 120.0), (5, 120.0), (10, 120.0)],
    )
    .expect("T1 hazard calibration");

    let make_market_state =
        |disc: DiscountCurve, haz: HazardCurve, prices: BTreeMap<String, MarketScalar>| {
            MarketContextState {
                schema_version: finstack_quant_core::wire::SchemaVersion::CURRENT,
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
    use finstack_quant_attribution::TaylorAttributionConfig;

    let as_of_t0 = create_date(2025, Month::January, 1).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 2).unwrap();

    let mut bond = Bond::fixed(
        "BOND-ISSUER-A",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05_f64).expect("valid rate fixture"),
        create_date(2024, Month::January, 1).unwrap(),
        create_date(2034, Month::January, 1).unwrap(),
        finstack_quant_core::dates::StubKind::ShortFront,
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

    // Use lossless quote recipes on both dates: T0 is flat, while T1 has a
    // pronounced short-end widening and long-end tightening.
    let disc_t0 = make_discount(as_of_t0);
    let disc_t1 = make_discount(as_of_t1);
    let convention = CdsConventionKey {
        currency: Currency::USD,
        doc_clause: CdsDocClause::IsdaNa,
    };
    let haz_t0 = calibrated_hazard_curve(
        &disc_t0,
        as_of_t0,
        "ISSUER-A-HAZ",
        "ISSUER-A",
        0.4,
        convention.clone(),
        &[
            (1, 120.0),
            (2, 120.0),
            (3, 120.0),
            (5, 120.0),
            (7, 120.0),
            (10, 120.0),
            (30, 120.0),
        ],
    )
    .expect("T0 hazard calibration");
    let haz_t1 = calibrated_hazard_curve(
        &disc_t1,
        as_of_t1,
        "ISSUER-A-HAZ",
        "ISSUER-A",
        0.4,
        convention,
        &[
            (1, 180.0),
            (2, 165.0),
            (3, 150.0),
            (5, 135.0),
            (7, 125.0),
            (10, 115.0),
            (30, 110.0),
        ],
    )
    .expect("T1 hazard calibration");

    let make_market_state =
        |disc: DiscountCurve, haz: HazardCurve, prices: BTreeMap<String, MarketScalar>| {
            MarketContextState {
                schema_version: finstack_quant_core::wire::SchemaVersion::CURRENT,
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
        market_t0: make_market_state(disc_t0, haz_t0, prices()),
        market_t1: make_market_state(disc_t1, haz_t1, prices()),
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
