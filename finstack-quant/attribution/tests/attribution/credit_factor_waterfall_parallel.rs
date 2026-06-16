//! PR-8a: credit-factor hierarchy detail for waterfall + parallel attribution.
//!
//! Four named tests:
//!  1. `waterfall_credit_factor_detail_reconciles_to_credit_curves_pnl`
//!  2. `parallel_credit_detail_plus_cross_effects_preserves_total`
//!  3. `waterfall_no_model_keeps_default_credit_step`
//!  4. `same_credit_total_different_hierarchy_different_detail`

use finstack_quant_attribution::{
    default_waterfall_order, AttributionEnvelope, AttributionMethod, AttributionSpec,
    CreditFactorDetailOptions,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{create_date, DayCount};
use finstack_quant_core::market_data::context::{
    CurveState, MarketContextState, MARKET_CONTEXT_STATE_VERSION,
};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, IssuerId};
use finstack_quant_factor_model::credit::hierarchy::{
    AdderVolSource, CalibrationDiagnostics, CreditFactorModel, CreditHierarchySpec, DateRange,
    FactorCorrelationMatrix, GenericFactorSpec, HierarchyDimension, IssuerBetaMode,
    IssuerBetaPolicy, IssuerBetaRow, IssuerBetas, IssuerTags, LevelsAtAnchor, VolState,
};
use finstack_quant_factor_model::{
    FactorCovarianceMatrix, FactorModelConfig, MatchingConfig, PricingMode,
};
use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
use finstack_quant_valuations::instruments::{Attributes, Bond};
use std::collections::BTreeMap;
use time::Month;

// ─────────────────────────── Helpers ───────────────────────────

fn issuer_tags(rating: &str, region: &str) -> IssuerTags {
    let mut m = BTreeMap::new();
    m.insert("rating".into(), rating.into());
    m.insert("region".into(), region.into());
    // Carry sector too so the same issuer can be reused with sector-aware
    // hierarchies in tests that vary the level set.
    m.insert("sector".into(), "FIN".into());
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

fn make_model(levels: Vec<HierarchyDimension>) -> CreditFactorModel {
    let n = levels.len();
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
        hierarchy: CreditHierarchySpec { levels },
        config: empty_factor_config(),
        issuer_betas: vec![issuer_row(
            "ISSUER-A",
            "IG",
            "EU",
            1.10,
            vec![0.90; n.max(1)],
        )],
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

fn make_bond() -> Bond {
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
    bond
}

fn flat_discount(base: time::Date) -> DiscountCurve {
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
}

fn flat_hazard(base: time::Date, rate: f64) -> HazardCurve {
    HazardCurve::builder("ISSUER-A-HAZ")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .recovery_rate(0.4)
        .knots([(0.5_f64, rate), (5.0_f64, rate), (10.0_f64, rate)])
        .build()
        .expect("hazard curve")
}

fn make_market_state(disc: DiscountCurve, haz: HazardCurve) -> MarketContextState {
    MarketContextState {
        version: MARKET_CONTEXT_STATE_VERSION,
        curves: vec![CurveState::Discount(disc), CurveState::Hazard(haz)],
        fx: None,
        surfaces: vec![],
        prices: BTreeMap::new(),
        series: vec![],
        inflation_indices: vec![],
        dividends: vec![],
        credit_indices: vec![],
        collateral: BTreeMap::new(),
        fx_delta_vol_surfaces: vec![],
        hierarchy: None,
        vol_cubes: vec![],
    }
}

fn standard_period() -> (time::Date, time::Date) {
    (
        create_date(2025, Month::January, 1).unwrap(),
        create_date(2025, Month::January, 2).unwrap(),
    )
}

/// Hazard curve with knots at the standard tenor grid set to the given rates,
/// used to build a deliberately non-parallel (twisted) hazard move.
fn twisted_hazard(base: time::Date, rates: &[f64]) -> HazardCurve {
    let std_tenors = [0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 30.0];
    HazardCurve::builder("ISSUER-A-HAZ")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .recovery_rate(0.4)
        .knots(
            std_tenors
                .iter()
                .zip(rates.iter())
                .map(|(&t, &r)| (t, r))
                .collect::<Vec<_>>(),
        )
        .build()
        .expect("twisted hazard curve")
}

// ─────────────────────────── Tests ───────────────────────────

/// PR-8a test 1: waterfall reconciliation invariant.
#[test]
fn waterfall_credit_factor_detail_reconciles_to_credit_curves_pnl() {
    let (as_of_t0, as_of_t1) = standard_period();
    let bond = make_bond();
    let model = make_model(vec![HierarchyDimension::Rating, HierarchyDimension::Region]);

    let market_t0 = make_market_state(flat_discount(as_of_t0), flat_hazard(as_of_t0, 0.01));
    let market_t1 = make_market_state(flat_discount(as_of_t1), flat_hazard(as_of_t1, 0.02));

    let spec = AttributionSpec {
        instrument: InstrumentJson::Bond(bond),
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
        method: AttributionMethod::Waterfall(default_waterfall_order()),
        model_params_t0: None,
        credit_factor_model: Some(Box::new(model)),
        credit_factor_detail_options: CreditFactorDetailOptions::default(),
        config: None,
        full_cross_attribution: false,
    };

    let result = AttributionEnvelope::new(spec)
        .execute()
        .expect("waterfall attribution should succeed");
    let attribution = result.result.attribution;

    let detail = attribution
        .credit_factor_detail
        .as_ref()
        .expect("credit_factor_detail must be Some for waterfall + model");

    // Reconciliation invariant (audit item #1): the non-parallel hazard
    // residual is now its own `curve_shape_pnl` component, so the closing
    // sum is generic + Σlevels + adder + curve_shape.
    let attributed = detail.generic_pnl.amount()
        + detail.levels.iter().map(|l| l.total.amount()).sum::<f64>()
        + detail.adder_pnl_total.amount()
        + detail.curve_shape_pnl.amount();
    let expected = attribution.credit_curves_pnl.amount();
    assert!(
        (attributed - expected).abs() < 1e-8,
        "waterfall reconciliation: attributed={attributed}, credit_curves_pnl={expected}"
    );
    // For a flat (parallel) hazard move the curve-shape component is ~0.
    assert!(
        detail.curve_shape_pnl.amount().abs() < 1e-6,
        "a flat hazard move must leave curve_shape ~0, got {}",
        detail.curve_shape_pnl.amount()
    );
}

/// Audit item #1: a NON-PARALLEL (twisted) hazard-curve move must be
/// attributed to the `curve_shape` component, not absorbed into the per-issuer
/// adder. With the standard tenors flat at 200 bp at T0 and a twist at T1
/// (short tenors up, long tenors down, signed average ≈ 0), the parallel
/// cascade steps (generic / level / adder) see essentially no parallel move,
/// so almost all of the credit P&L must land in `curve_shape_pnl`.
#[test]
fn waterfall_twisted_hazard_attributes_curve_shape_not_adder() {
    let (as_of_t0, as_of_t1) = standard_period();
    let bond = make_bond();
    let model = make_model(vec![HierarchyDimension::Rating, HierarchyDimension::Region]);

    // T0 flat at 200 bp; T1 twisted with shifts (+100,+100,+50,+50,0,-50,-50,
    // -100,-100) bp — signed average exactly 0, large L1.
    let market_t0 = make_market_state(
        flat_discount(as_of_t0),
        twisted_hazard(as_of_t0, &[0.02; 9]),
    );
    let market_t1 = make_market_state(
        flat_discount(as_of_t1),
        twisted_hazard(
            as_of_t1,
            &[
                0.030, 0.030, 0.025, 0.025, 0.020, 0.015, 0.015, 0.010, 0.010,
            ],
        ),
    );

    let spec = AttributionSpec {
        instrument: InstrumentJson::Bond(bond),
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
        method: AttributionMethod::Waterfall(default_waterfall_order()),
        model_params_t0: None,
        credit_factor_model: Some(Box::new(model)),
        credit_factor_detail_options: CreditFactorDetailOptions::default(),
        config: None,
        full_cross_attribution: false,
    };

    let result = AttributionEnvelope::new(spec)
        .execute()
        .expect("waterfall attribution should succeed");
    let attribution = result.result.attribution;
    let detail = attribution
        .credit_factor_detail
        .as_ref()
        .expect("credit_factor_detail must be Some for waterfall + model");

    // Reconciliation still closes with the curve-shape component included.
    let attributed = detail.generic_pnl.amount()
        + detail.levels.iter().map(|l| l.total.amount()).sum::<f64>()
        + detail.adder_pnl_total.amount()
        + detail.curve_shape_pnl.amount();
    let expected = attribution.credit_curves_pnl.amount();
    assert!(
        (attributed - expected).abs() < 1e-8,
        "reconciliation must hold: attributed={attributed}, credit_curves_pnl={expected}"
    );

    // The credit P&L is non-trivial (the curve genuinely moved).
    assert!(
        expected.abs() > 1.0,
        "twisted curve must produce a material credit P&L, got {expected}"
    );

    // The curve-shape component must carry essentially all of it: the parallel
    // generic / level / adder steps see a ~0 signed move.
    let parallel_part = detail.generic_pnl.amount()
        + detail.levels.iter().map(|l| l.total.amount()).sum::<f64>()
        + detail.adder_pnl_total.amount();
    assert!(
        detail.curve_shape_pnl.amount().abs() > parallel_part.abs(),
        "curve-shape component ({}) must dominate the parallel components ({}) \
         for a twisted hazard move",
        detail.curve_shape_pnl.amount(),
        parallel_part
    );
    // And specifically: the adder must NOT be where the curve-shape risk went.
    assert!(
        detail.adder_pnl_total.amount().abs() < detail.curve_shape_pnl.amount().abs(),
        "non-parallel risk must land in curve_shape ({}), not the adder ({})",
        detail.curve_shape_pnl.amount(),
        detail.adder_pnl_total.amount()
    );
}

/// PR-8a test 2: parallel reconciliation including cross-effects.
#[test]
fn parallel_credit_detail_plus_cross_effects_preserves_total() {
    let (as_of_t0, as_of_t1) = standard_period();
    let bond = make_bond();
    let model = make_model(vec![HierarchyDimension::Rating, HierarchyDimension::Region]);

    let market_t0 = make_market_state(flat_discount(as_of_t0), flat_hazard(as_of_t0, 0.01));
    let market_t1 = make_market_state(flat_discount(as_of_t1), flat_hazard(as_of_t1, 0.02));

    let spec = AttributionSpec {
        instrument: InstrumentJson::Bond(bond),
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
        method: AttributionMethod::Parallel,
        model_params_t0: None,
        credit_factor_model: Some(Box::new(model)),
        credit_factor_detail_options: CreditFactorDetailOptions::default(),
        config: None,
        full_cross_attribution: false,
    };

    let result = AttributionEnvelope::new(spec)
        .execute()
        .expect("parallel attribution should succeed");
    let attribution = result.result.attribution;

    let detail = attribution
        .credit_factor_detail
        .as_ref()
        .expect("credit_factor_detail must be Some for parallel + model");

    // Audit item #1: the parallel path now back-solves the non-parallel
    // hazard residual into the `curve_shape_pnl` cascade component, so
    // `generic + Σlevels + adder + curve_shape ≡ credit_curves_pnl` closes
    // exactly — there is no longer a `CreditCascadeResidual` cross-factor
    // entry to add back in.
    let credit_detail_total = detail.generic_pnl.amount()
        + detail.levels.iter().map(|l| l.total.amount()).sum::<f64>()
        + detail.adder_pnl_total.amount()
        + detail.curve_shape_pnl.amount();

    assert!(
        attribution
            .cross_factor_detail
            .as_ref()
            .and_then(|d| d.by_pair.get("CreditCascadeResidual"))
            .is_none(),
        "CreditCascadeResidual cross-effect is replaced by the curve_shape component"
    );

    let expected = attribution.credit_curves_pnl.amount();
    assert!(
        (credit_detail_total - expected).abs() < 1e-6,
        "parallel reconciliation: detail={credit_detail_total}, credit_curves_pnl={expected}"
    );
}

/// PR-8a test 3: no-model waterfall keeps the legacy single Credit step.
/// The default factor order length and credit-step P&L stay unchanged byte-
/// identical between two runs that omit the credit factor model.
#[test]
fn waterfall_no_model_keeps_default_credit_step() {
    // Default order is length 9 with CreditCurves at index 2.
    let order = default_waterfall_order();
    assert_eq!(order.len(), 9);

    let (as_of_t0, as_of_t1) = standard_period();
    let bond = make_bond();
    let market_t0 = make_market_state(flat_discount(as_of_t0), flat_hazard(as_of_t0, 0.01));
    let market_t1 = make_market_state(flat_discount(as_of_t1), flat_hazard(as_of_t1, 0.02));

    let spec = AttributionSpec {
        instrument: InstrumentJson::Bond(bond),
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
        method: AttributionMethod::Waterfall(default_waterfall_order()),
        model_params_t0: None,
        credit_factor_model: None,
        credit_factor_detail_options: CreditFactorDetailOptions::default(),
        config: None,
        full_cross_attribution: false,
    };

    let result = AttributionEnvelope::new(spec)
        .execute()
        .expect("waterfall attribution should succeed");
    let attribution = result.result.attribution;

    // No credit factor detail when no model.
    assert!(attribution.credit_factor_detail.is_none());
    // Credit step still produced a value (non-zero).
    assert!(attribution.credit_curves_pnl.amount().abs() > 0.0);
}

#[test]
fn parallel_model_with_unmapped_issuer_adds_diagnostic_note() {
    let (as_of_t0, as_of_t1) = standard_period();
    let bond = make_bond();
    let mut model = make_model(vec![HierarchyDimension::Rating]);
    model.issuer_betas.clear();

    let spec = AttributionSpec {
        instrument: InstrumentJson::Bond(bond),
        market_t0: make_market_state(flat_discount(as_of_t0), flat_hazard(as_of_t0, 0.01)),
        market_t1: make_market_state(flat_discount(as_of_t1), flat_hazard(as_of_t1, 0.02)),
        as_of_t0,
        as_of_t1,
        method: AttributionMethod::Parallel,
        model_params_t0: None,
        credit_factor_model: Some(Box::new(model)),
        credit_factor_detail_options: CreditFactorDetailOptions::default(),
        config: None,
        full_cross_attribution: false,
    };

    let attribution = AttributionEnvelope::new(spec)
        .execute()
        .expect("parallel attribution should succeed")
        .result
        .attribution;

    assert!(attribution.credit_factor_detail.is_none());
    assert!(
        attribution
            .meta
            .notes
            .iter()
            .any(|note| note.contains("credit_factor_model supplied")
                && note.contains("credit_factor_detail omitted")),
        "unmapped issuer should be visible in notes: {:?}",
        attribution.meta.notes
    );
}

/// PR-8a test 4: same credit total, different hierarchies → different details.
#[test]
fn same_credit_total_different_hierarchy_different_detail() {
    let (as_of_t0, as_of_t1) = standard_period();

    let market_t0 = make_market_state(flat_discount(as_of_t0), flat_hazard(as_of_t0, 0.01));
    let market_t1 = make_market_state(flat_discount(as_of_t1), flat_hazard(as_of_t1, 0.02));

    let run = |levels: Vec<HierarchyDimension>| {
        let bond = make_bond();
        let model = make_model(levels);
        let spec = AttributionSpec {
            instrument: InstrumentJson::Bond(bond),
            market_t0: market_t0.clone(),
            market_t1: market_t1.clone(),
            as_of_t0,
            as_of_t1,
            method: AttributionMethod::Waterfall(default_waterfall_order()),
            model_params_t0: None,
            credit_factor_model: Some(Box::new(model)),
            credit_factor_detail_options: CreditFactorDetailOptions::default(),
            config: None,
            full_cross_attribution: false,
        };
        AttributionEnvelope::new(spec)
            .execute()
            .expect("waterfall attribution should succeed")
            .result
            .attribution
    };

    let a = run(vec![HierarchyDimension::Rating, HierarchyDimension::Region]);
    let b = run(vec![HierarchyDimension::Sector]);

    // Same total credit P&L (waterfall snaps to T1 hazard at adder step).
    assert!(
        (a.credit_curves_pnl.amount() - b.credit_curves_pnl.amount()).abs() < 1e-8,
        "credit_curves_pnl should be identical: a={}, b={}",
        a.credit_curves_pnl.amount(),
        b.credit_curves_pnl.amount()
    );

    let detail_a = a.credit_factor_detail.as_ref().unwrap();
    let detail_b = b.credit_factor_detail.as_ref().unwrap();
    // Different hierarchy depths → different number of LevelPnl entries.
    assert_ne!(detail_a.levels.len(), detail_b.levels.len());

    // Both reconcile to credit_curves_pnl.
    for attribution in [&a, &b] {
        let detail = attribution.credit_factor_detail.as_ref().unwrap();
        let attributed = detail.generic_pnl.amount()
            + detail.levels.iter().map(|l| l.total.amount()).sum::<f64>()
            + detail.adder_pnl_total.amount();
        assert!(
            (attributed - attribution.credit_curves_pnl.amount()).abs() < 1e-8,
            "reconciliation failed for one of the runs"
        );
    }
}

/// Regression (audit C1 → cumulative cascade): the parallel credit cascade
/// uses **cumulative bumps** that telescope to `credit_curves_pnl`, mirroring
/// the waterfall cascade. The step P&L is the marginal `V_k − V_{k−1}` along
/// the chain `base → +bp_1 → +bp_1+bp_2 → … → snap(T1)`. For a perfectly
/// parallel hazard move, only the generic step does meaningful work — the
/// subsequent levels / adder add 0 bp, and the curve-shape snap lands on the
/// same hazard state the cumulative bumps already produced — so
/// `curve_shape_pnl ≈ 0` and `parallel.generic_pnl ≡ waterfall.generic_pnl`.
///
/// A buggy non-telescoping formulation (`val_t1 − V_step` or
/// `V_step − base` summed without cumulative chaining) would either drive
/// every moved factor's P&L toward zero, dump a spurious offset into
/// `curve_shape_pnl`, or — in the CS-gamma case — silently route cross-bp
/// convexity into `curve_shape_pnl` and trip the curve-shape tracing warn
/// on a perfectly parallel hazard move.
#[test]
fn parallel_credit_cascade_attributes_each_step_to_its_own_contribution() {
    let (as_of_t0, as_of_t1) = standard_period();
    let market_t0 = make_market_state(flat_discount(as_of_t0), flat_hazard(as_of_t0, 0.01));
    let market_t1 = make_market_state(flat_discount(as_of_t1), flat_hazard(as_of_t1, 0.02));

    let run = |method: AttributionMethod| {
        let spec = AttributionSpec {
            instrument: InstrumentJson::Bond(make_bond()),
            market_t0: market_t0.clone(),
            market_t1: market_t1.clone(),
            as_of_t0,
            as_of_t1,
            method,
            model_params_t0: None,
            credit_factor_model: Some(Box::new(make_model(vec![
                HierarchyDimension::Rating,
                HierarchyDimension::Region,
            ]))),
            credit_factor_detail_options: CreditFactorDetailOptions::default(),
            config: None,
            full_cross_attribution: false,
        };
        AttributionEnvelope::new(spec)
            .execute()
            .expect("attribution should succeed")
            .result
            .attribution
    };

    let parallel = run(AttributionMethod::Parallel);
    let waterfall = run(AttributionMethod::Waterfall(default_waterfall_order()));

    let p = parallel
        .credit_factor_detail
        .as_ref()
        .expect("parallel credit detail");
    let w = waterfall
        .credit_factor_detail
        .as_ref()
        .expect("waterfall credit detail");
    let credit_pnl = parallel.credit_curves_pnl.amount();
    assert!(
        credit_pnl.abs() > 1.0,
        "flat hazard move must produce material credit P&L, got {credit_pnl}"
    );

    // (1) A flat (parallel) hazard move has essentially no curve-shape residual.
    // The buggy complement formula instead produced curve_shape ≈ -2 × credit_pnl.
    assert!(
        p.curve_shape_pnl.amount().abs() <= 1e-6 + 0.05 * credit_pnl.abs(),
        "flat move must leave curve_shape ~0, got {} vs credit_pnl {credit_pnl}",
        p.curve_shape_pnl.amount()
    );

    // (2) The parallel bp-bump steps must themselves sum to ~credit_pnl, not
    // ~N × credit_pnl as the complement formula produced.
    let parallel_part = p.generic_pnl.amount()
        + p.levels.iter().map(|l| l.total.amount()).sum::<f64>()
        + p.adder_pnl_total.amount();
    assert!(
        (parallel_part - credit_pnl).abs() <= 1e-6 + 0.05 * credit_pnl.abs(),
        "parallel steps must sum to ~credit_pnl, got {parallel_part} vs {credit_pnl}"
    );

    // (3) The generic step reprices an identical market in both methods (a
    // generic-sized bump applied to the same T0-hazard base), so its P&L must
    // match the waterfall generic component. The buggy complement formula made
    // the parallel generic component the full credit P&L instead.
    assert!(
        (p.generic_pnl.amount() - w.generic_pnl.amount()).abs()
            <= 1e-6 + 1e-6 * w.generic_pnl.amount().abs(),
        "parallel generic_pnl {} must match waterfall generic_pnl {}",
        p.generic_pnl.amount(),
        w.generic_pnl.amount()
    );

    // Sum still reconciles (the back-solve guarantees this with or without the
    // bug — kept as a sanity check, not the regression guard).
    assert!(
        (parallel_part + p.curve_shape_pnl.amount() - credit_pnl).abs() < 1e-6,
        "credit_factor_detail must reconcile to credit_curves_pnl"
    );
}

/// Regression: with the discount curve drifting between T0 and T1, the
/// linear-path CS01 (used by metrics-based / Taylor) must be measured at the
/// SAME baseline as `credit_curves_pnl` — `market_t1` with the issuer's
/// hazard curves restored to T0, priced at `as_of_t1`. Before the M2 fix the
/// linear path measured CS01 at (market_t0, as_of_t0), and any rate drift
/// silently distorted the generic / level / adder split.
///
/// This test exercises a 100bp parallel rate move alongside a 100bp parallel
/// hazard move. With the fix in place, the metrics-based decomposition must
/// reconcile to `credit_curves_pnl` AND its `generic_pnl` must agree closely
/// with the parallel cascade's `generic_pnl` (which uses the correct baseline
/// natively via cumulative bumps).
#[test]
fn metrics_based_credit_factor_detail_uses_t1_cs01_baseline() {
    let (as_of_t0, as_of_t1) = standard_period();
    let bond = make_bond();
    let model = make_model(vec![HierarchyDimension::Rating, HierarchyDimension::Region]);

    // Rate drift: 5% → 6% (100 bp parallel).
    let drift_discount = |base: time::Date, r: f64| {
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
    let market_t0 = make_market_state(drift_discount(as_of_t0, 0.05), flat_hazard(as_of_t0, 0.01));
    let market_t1 = make_market_state(drift_discount(as_of_t1, 0.06), flat_hazard(as_of_t1, 0.02));

    let run = |method: AttributionMethod| {
        let spec = AttributionSpec {
            instrument: InstrumentJson::Bond(bond.clone()),
            market_t0: market_t0.clone(),
            market_t1: market_t1.clone(),
            as_of_t0,
            as_of_t1,
            method,
            model_params_t0: None,
            credit_factor_model: Some(Box::new(model.clone())),
            credit_factor_detail_options: CreditFactorDetailOptions::default(),
            config: None,
            full_cross_attribution: false,
        };
        AttributionEnvelope::new(spec)
            .execute()
            .expect("attribution should succeed")
            .result
            .attribution
    };

    let metrics = run(AttributionMethod::MetricsBased);
    let parallel = run(AttributionMethod::Parallel);

    let m = metrics
        .credit_factor_detail
        .as_ref()
        .expect("metrics-based credit detail");
    let p = parallel
        .credit_factor_detail
        .as_ref()
        .expect("parallel credit detail");

    // Reconciliation: generic + Σlevels + adder + curve_shape ≡ credit_curves_pnl
    // — must hold even with the rate drift (M2 fix preserved this exactly).
    let m_attributed = m.generic_pnl.amount()
        + m.levels.iter().map(|l| l.total.amount()).sum::<f64>()
        + m.adder_pnl_total.amount()
        + m.curve_shape_pnl.amount();
    let m_total = metrics.credit_curves_pnl.amount();
    assert!(
        (m_attributed - m_total).abs() < 1e-8,
        "metrics-based reconciliation: attributed={m_attributed}, credit={m_total}"
    );

    // Substantive check: the metrics-based CS01 (now measured against T1
    // markets with T0 hazard) and the parallel cascade's CS01 (measured at
    // T1 markets natively) should agree on the per-factor split to a tight
    // tolerance. Before M2, the rate drift would have pushed an O(ΔDV01 ×
    // Δs) wedge between them and shifted that wedge into curve_shape.
    let m_generic = m.generic_pnl.amount();
    let p_generic = p.generic_pnl.amount();
    let agree = |a: f64, b: f64| {
        let scale = m_total.abs().max(1.0);
        (a - b).abs() <= 5e-3 * scale
    };
    assert!(
        agree(m_generic, p_generic),
        "metrics-based generic_pnl {m_generic} must agree with parallel generic_pnl {p_generic} \
         to within 0.5% of credit_pnl {m_total} (M2: same CS01 baseline)"
    );
}
