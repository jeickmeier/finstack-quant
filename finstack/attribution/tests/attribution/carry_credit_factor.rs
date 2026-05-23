//! PR-8b: carry decomposition under a calibrated `CreditFactorModel`.
//!
//! Required tests (spec §10.4 / PR-8b):
//!   1. `carry_coupon_total_equals_rates_plus_credit`
//!   2. `carry_roll_down_total_equals_rates_plus_credit`
//!   3. `credit_carry_total_equals_sum_of_credit_source_lines`
//!   4. `credit_carry_total_equals_generic_levels_and_adder`
//!   5. `rates_carry_total_matches_rates_source_lines_minus_funding`
//!   6. `carry_no_model_keeps_scalar_source_lines`
//!   7. `carry_credit_roll_down_all_to_adder` (per spec §7.3 v1)

use finstack_attribution::{
    AttributionConfig, AttributionEnvelope, AttributionMethod, AttributionSpec,
    CreditFactorDetailOptions, PnlAttribution,
};
use finstack_core::currency::Currency;
use finstack_core::dates::{create_date, DayCount};
use finstack_core::factor_model::credit_hierarchy::{
    AdderVolSource, CalibrationDiagnostics, CreditFactorModel, CreditHierarchySpec, DateRange,
    FactorCorrelationMatrix, GenericFactorSpec, HierarchyDimension, IssuerBetaMode,
    IssuerBetaPolicy, IssuerBetaRow, IssuerBetas, IssuerTags, LevelAnchor, LevelsAtAnchor,
    VolState,
};
use finstack_core::factor_model::{
    FactorCovarianceMatrix, FactorModelConfig, MatchingConfig, PricingMode,
};
use finstack_core::market_data::context::{
    CurveState, MarketContextState, MARKET_CONTEXT_STATE_VERSION,
};
use finstack_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_core::money::Money;
use finstack_core::types::{CurveId, IssuerId};
use finstack_valuations::instruments::json_loader::InstrumentJson;
use finstack_valuations::instruments::{Attributes, Bond};
use std::collections::BTreeMap;
use time::Month;

const TOL: f64 = 1e-8;

// ───────────────────────── Model & market helpers ─────────────────────────

fn make_tags(rating: &str, region: &str) -> IssuerTags {
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

fn issuer_row(
    id: &str,
    rating: &str,
    region: &str,
    pc: f64,
    levels: Vec<f64>,
    adder: f64,
) -> IssuerBetaRow {
    IssuerBetaRow {
        issuer_id: IssuerId::new(id),
        tags: make_tags(rating, region),
        mode: IssuerBetaMode::IssuerBeta,
        betas: IssuerBetas { pc, levels },
        adder_at_anchor: adder,
        adder_vol_annualized: 0.005,
        adder_vol_source: AdderVolSource::Default,
        fit_quality: None,
    }
}

fn make_model() -> CreditFactorModel {
    // Anchor state: PC=0.005 (50bp), level0(IG)=0.003 (30bp), level1(EU)=0.002.
    // Issuer beta: pc=1.1, levels=[0.9, 1.05]. Adder=0.0008 (8bp).
    // Implied issuer S = 1.1*0.005 + 0.9*0.003 + 1.05*0.002 + 0.0008
    //                  = 0.0055 + 0.0027 + 0.0021 + 0.0008 = 0.0111 (~111 bp).
    let mut by_level = Vec::new();
    let mut rating_values = BTreeMap::new();
    rating_values.insert("IG".into(), 0.003_f64);
    rating_values.insert("HY".into(), 0.012_f64);
    by_level.push(LevelAnchor {
        level_index: 0,
        dimension: HierarchyDimension::Rating,
        values: rating_values,
    });
    let mut region_values = BTreeMap::new();
    region_values.insert("IG.EU".into(), 0.002_f64);
    region_values.insert("IG.NA".into(), 0.0025_f64);
    region_values.insert("HY.NA".into(), 0.005_f64);
    by_level.push(LevelAnchor {
        level_index: 1,
        dimension: HierarchyDimension::Region,
        values: region_values,
    });

    CreditFactorModel {
        schema_version: CreditFactorModel::SCHEMA_VERSION.into(),
        as_of: create_date(2024, Month::December, 31).unwrap(),
        calibration_window: DateRange {
            start: create_date(2022, Month::December, 31).unwrap(),
            end: create_date(2024, Month::December, 31).unwrap(),
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
        issuer_betas: vec![issuer_row(
            "ISSUER-A",
            "IG",
            "EU",
            1.1,
            vec![0.9, 1.05],
            0.0008,
        )],
        anchor_state: LevelsAtAnchor {
            pc: 0.005,
            by_level,
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

fn build_bond_with_issuer() -> Bond {
    let mut bond = Bond::fixed(
        "BOND-ISSUER-A",
        Money::new(1_000_000.0, Currency::USD),
        0.05_f64,
        create_date(2024, Month::January, 1).unwrap(),
        create_date(2034, Month::January, 1).unwrap(),
        "USD-OIS",
    )
    .expect("bond");
    bond.credit_curve_id = Some(CurveId::new("ISSUER-A-HAZ"));
    bond.attributes = Attributes::new().with_meta("credit::issuer_id", "ISSUER-A");
    bond
}

fn flat_discount(base: time::Date, r: f64) -> DiscountCurve {
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

fn flat_hazard(base: time::Date, h: f64) -> HazardCurve {
    HazardCurve::builder("ISSUER-A-HAZ")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .recovery_rate(0.4)
        .knots([(0.5_f64, h), (5.0_f64, h), (10.0_f64, h)])
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

fn run_metrics_based_with_model(model: Option<CreditFactorModel>) -> PnlAttribution {
    let as_of_t0 = create_date(2025, Month::January, 1).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 31).unwrap();
    let bond = build_bond_with_issuer();
    let disc_t0 = flat_discount(as_of_t0, 0.05);
    let disc_t1 = flat_discount(as_of_t1, 0.05);
    let haz_t0 = flat_hazard(as_of_t0, 0.011); // 110 bp ≈ implied issuer S in model
    let haz_t1 = flat_hazard(as_of_t1, 0.012); // small +10 bp move
    let credit_factor_model = model.map(Box::new);
    // Request carry-decomposition metrics so MetricsBased populates
    // coupon_income / pull_to_par / roll_down / funding_cost.
    let metrics = vec![
        "theta".to_string(),
        "dv01".to_string(),
        "cs01".to_string(),
        "carry_total".to_string(),
        "coupon_income".to_string(),
        "pull_to_par".to_string(),
        "roll_down".to_string(),
        "funding_cost".to_string(),
    ];
    let spec = AttributionSpec {
        instrument: InstrumentJson::Bond(bond),
        market_t0: make_market_state(disc_t0, haz_t0),
        market_t1: make_market_state(disc_t1, haz_t1),
        as_of_t0,
        as_of_t1,
        method: AttributionMethod::MetricsBased,
        model_params_t0: None,
        credit_factor_model,
        credit_factor_detail_options: CreditFactorDetailOptions::default(),
        config: Some(AttributionConfig {
            tolerance_abs: None,
            tolerance_pct: None,
            metrics: Some(metrics),
            strict_validation: None,
            rounding_scale: None,
            rate_bump_bp: None,
        }),
        full_cross_attribution: false,
    };
    AttributionEnvelope::new(spec)
        .execute()
        .expect("attribution should succeed")
        .result
        .attribution
}

// ───────────────────────────── Tests ─────────────────────────────

/// Invariant 1: `coupon_income.total ≡ rates_part + credit_part` when a model
/// is supplied (§7.4).
#[test]
fn carry_coupon_total_equals_rates_plus_credit() {
    let attribution = run_metrics_based_with_model(Some(make_model()));
    let detail = attribution
        .carry_detail
        .as_ref()
        .expect("carry_detail populated");
    let coupon = detail
        .coupon_income
        .as_ref()
        .expect("coupon_income populated under model");
    let rates = coupon
        .rates_part
        .expect("rates_part populated under model")
        .amount();
    let credit = coupon
        .credit_part
        .expect("credit_part populated under model")
        .amount();
    assert!(
        (coupon.total.amount() - (rates + credit)).abs() < TOL,
        "coupon_income split failed: total={}, rates+credit={}",
        coupon.total.amount(),
        rates + credit
    );
}

/// Invariant 2: `roll_down.total ≡ rates_part + credit_part` (§7.4).
#[test]
fn carry_roll_down_total_equals_rates_plus_credit() {
    let attribution = run_metrics_based_with_model(Some(make_model()));
    let detail = attribution
        .carry_detail
        .as_ref()
        .expect("carry_detail populated");
    let roll = detail
        .roll_down
        .as_ref()
        .expect("roll_down populated under model");
    let rates = roll
        .rates_part
        .expect("rates_part populated under model")
        .amount();
    let credit = roll
        .credit_part
        .expect("credit_part populated under model")
        .amount();
    assert!(
        (roll.total.amount() - (rates + credit)).abs() < TOL,
        "roll_down split failed: total={}, rates+credit={}",
        roll.total.amount(),
        rates + credit
    );
}

/// Invariant 3: `credit_carry_total ≡ Σ_lines SourceLine.credit_part`
/// where lines = coupon_income + roll_down (§7.4). pull_to_par is unsplit.
#[test]
fn credit_carry_total_equals_sum_of_credit_source_lines() {
    let attribution = run_metrics_based_with_model(Some(make_model()));
    let detail = attribution
        .carry_detail
        .as_ref()
        .expect("carry_detail populated");
    let cc = attribution
        .credit_carry_decomposition
        .as_ref()
        .expect("credit_carry_decomposition populated");

    let coupon_credit = detail
        .coupon_income
        .as_ref()
        .and_then(|l| l.credit_part)
        .map(|m| m.amount())
        .unwrap_or(0.0);
    let roll_credit = detail
        .roll_down
        .as_ref()
        .and_then(|l| l.credit_part)
        .map(|m| m.amount())
        .unwrap_or(0.0);
    let sum_credit = coupon_credit + roll_credit;

    assert!(
        (cc.credit_carry_total.amount() - sum_credit).abs() < TOL,
        "credit_carry_total mismatch: total={}, Σ credit_parts={}",
        cc.credit_carry_total.amount(),
        sum_credit
    );
}

/// Invariant 4: `credit_carry_total ≡ generic + Σ_levels(level.total) + adder_total` (§7.4).
#[test]
fn credit_carry_total_equals_generic_levels_and_adder() {
    let attribution = run_metrics_based_with_model(Some(make_model()));
    let cc = attribution
        .credit_carry_decomposition
        .as_ref()
        .expect("credit_carry_decomposition populated");
    let by = &cc.credit_by_level;
    let recomposed = by.generic.amount()
        + by.levels.iter().map(|l| l.total.amount()).sum::<f64>()
        + by.adder_total.amount();
    assert!(
        (cc.credit_carry_total.amount() - recomposed).abs() < TOL,
        "factor-cut reconciliation failed: total={}, generic+levels+adder={}",
        cc.credit_carry_total.amount(),
        recomposed
    );
}

/// Invariant 5: `rates_carry_total ≡ Σ_lines SourceLine.rates_part − funding_cost` (§7.4).
#[test]
fn rates_carry_total_matches_rates_source_lines_minus_funding() {
    let attribution = run_metrics_based_with_model(Some(make_model()));
    let detail = attribution
        .carry_detail
        .as_ref()
        .expect("carry_detail populated");
    let cc = attribution
        .credit_carry_decomposition
        .as_ref()
        .expect("credit_carry_decomposition populated");

    let coupon_rates = detail
        .coupon_income
        .as_ref()
        .and_then(|l| l.rates_part)
        .map(|m| m.amount())
        .unwrap_or(0.0);
    let roll_rates = detail
        .roll_down
        .as_ref()
        .and_then(|l| l.rates_part)
        .map(|m| m.amount())
        .unwrap_or(0.0);
    let funding = detail
        .funding_cost
        .as_ref()
        .map(|m| m.amount())
        .unwrap_or(0.0);

    let expected = coupon_rates + roll_rates - funding;
    assert!(
        (cc.rates_carry_total.amount() - expected).abs() < TOL,
        "rates_carry_total mismatch: total={}, Σ rates_parts − funding={}",
        cc.rates_carry_total.amount(),
        expected
    );
}

/// No-model behavior: `SourceLine.rates_part` and `credit_part` are `None`,
/// no `credit_carry_decomposition` emitted (§7.1, additive contract).
#[test]
fn carry_no_model_keeps_scalar_source_lines() {
    let attribution = run_metrics_based_with_model(None);
    assert!(
        attribution.credit_carry_decomposition.is_none(),
        "credit_carry_decomposition should be None without a model"
    );
    if let Some(detail) = attribution.carry_detail.as_ref() {
        if let Some(coupon) = detail.coupon_income.as_ref() {
            assert!(
                coupon.rates_part.is_none(),
                "rates_part should be None without a model"
            );
            assert!(
                coupon.credit_part.is_none(),
                "credit_part should be None without a model"
            );
        }
        if let Some(roll) = detail.roll_down.as_ref() {
            assert!(
                roll.rates_part.is_none(),
                "rates_part should be None without a model"
            );
            assert!(
                roll.credit_part.is_none(),
                "credit_part should be None without a model"
            );
        }
    }
}

/// Per spec §7.3 v1: all credit roll-down → adder. Level factors are scalar
/// (no term-structure contribution), so for roll the level shares = 0 and
/// generic share = 0. We assert this by inspecting roll.credit_part itself —
/// it should be exactly zero under v1 since the model carries no adder term
/// structure (`adder_at(i, T) ≡ adder_at(i, T-dt)`). The rates_part absorbs
/// the entire roll_down.
#[test]
fn carry_credit_roll_down_all_to_adder() {
    let attribution = run_metrics_based_with_model(Some(make_model()));
    let detail = attribution
        .carry_detail
        .as_ref()
        .expect("carry_detail populated");
    let roll = detail
        .roll_down
        .as_ref()
        .expect("roll_down populated under model");
    let credit = roll
        .credit_part
        .expect("credit_part populated under model")
        .amount();
    assert!(
        credit.abs() < TOL,
        "v1: roll_down.credit_part should be zero (all credit roll → adder, \
         and adder has no term structure); got {credit}"
    );
}

/// Regression (Fix 1): when `s_model` is in the subnormal range `(0, 1e-15]`
/// (all betas = 0, anchor levels = 0, adder = 0), the adder fallback must
/// absorb `credit_total` so invariant 4 still holds at `TOL = 1e-8`.
///
/// Before Fix 1 the `s_for_scale != 0.0` check diverged from `s_model.abs() > 1e-15`,
/// leaving the adder as `0` and breaking invariant 4 for subnormal spreads.
#[test]
fn invariant4_holds_when_s_model_is_subnormal() {
    // Build a model where betas = 0 and adder = 0, so S_model = 0 exactly.
    let mut model = make_model();
    // Replace the single issuer row with one that has zero betas and zero adder.
    model.issuer_betas = vec![issuer_row("ISSUER-A", "IG", "EU", 0.0, vec![0.0, 0.0], 0.0)];

    let attribution = run_metrics_based_with_model(Some(model));
    let cc = attribution
        .credit_carry_decomposition
        .as_ref()
        .expect("credit_carry_decomposition populated even with zero s_model");
    let by = &cc.credit_by_level;
    let recomposed = by.generic.amount()
        + by.levels.iter().map(|l| l.total.amount()).sum::<f64>()
        + by.adder_total.amount();
    assert!(
        (cc.credit_carry_total.amount() - recomposed).abs() < TOL,
        "invariant 4 broken for subnormal s_model: total={}, generic+levels+adder={}",
        cc.credit_carry_total.amount(),
        recomposed
    );
}

/// Backward-compat: a JSON payload using the legacy `Money` shape for
/// `coupon_income` / `roll_down` deserializes into `SourceLine::scalar`.
///
/// We build it by serializing a current PnlAttribution then surgically
/// rewriting `carry_detail.coupon_income` / `roll_down` from the new
/// SourceLine shape (`{total: {amount,currency}, ...}`) to the legacy bare
/// Money shape (`{amount, currency}`).
#[test]
fn legacy_carry_detail_json_deserializes_into_scalar_source_line() {
    use finstack_attribution::{CarryDetail, SourceLine};
    use finstack_core::dates::create_date;

    let mut attr = PnlAttribution::new(
        Money::new(100.0, Currency::USD),
        "LEGACY",
        create_date(2025, time::Month::January, 1).unwrap(),
        create_date(2025, time::Month::January, 2).unwrap(),
        AttributionMethod::Parallel,
    );
    attr.carry = Money::new(30.0, Currency::USD);
    attr.carry_detail = Some(CarryDetail {
        total: Money::new(30.0, Currency::USD),
        coupon_income: Some(SourceLine::scalar(Money::new(25.0, Currency::USD))),
        pull_to_par: None,
        roll_down: Some(SourceLine::scalar(Money::new(5.0, Currency::USD))),
        funding_cost: None,
        theta: None,
    });

    // Serialize then mutate to legacy shape.
    let mut value = serde_json::to_value(&attr).expect("serialize");
    if let Some(carry) = value
        .get_mut("carry_detail")
        .and_then(|cd| cd.as_object_mut())
    {
        for key in ["coupon_income", "roll_down"] {
            if let Some(line) = carry.get(key).cloned() {
                if let Some(total) = line.get("total").cloned() {
                    // Replace with the legacy `Money` shape.
                    carry.insert(key.to_string(), total);
                }
            }
        }
    }
    let legacy_json = serde_json::to_string(&value).expect("re-serialize");

    let parsed: PnlAttribution =
        serde_json::from_str(&legacy_json).expect("legacy carry_detail JSON should parse");
    let detail = parsed.carry_detail.expect("carry_detail");
    let coupon = detail.coupon_income.expect("coupon");
    assert!((coupon.total.amount() - 25.0).abs() < TOL);
    assert!(coupon.rates_part.is_none());
    assert!(coupon.credit_part.is_none());
    let roll = detail.roll_down.expect("roll");
    assert!((roll.total.amount() - 5.0).abs() < TOL);
    assert!(roll.rates_part.is_none());
}
