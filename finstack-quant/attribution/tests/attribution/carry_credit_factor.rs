//! Carry decomposition under a calibrated `CreditFactorModel`.

use crate::attribution_support::calibrated_hazard_curve;
use finstack_quant_attribution::{
    AttributionConfig, AttributionEnvelope, AttributionMethod, AttributionSpec,
    CreditFactorDetailOptions, PnlAttribution,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{create_date, DayCount};
use finstack_quant_core::market_data::context::{CurveState, MarketContextState};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, IssuerId};
use finstack_quant_models::factor::credit::hierarchy::{
    AdderVolSource, CalibrationDiagnostics, CreditFactorModel, CreditHierarchySpec, DateRange,
    FactorCorrelationMatrix, GenericFactorSpec, HierarchyDimension, IssuerBetaMode,
    IssuerBetaPolicy, IssuerBetaRow, IssuerBetas, IssuerTags, LevelAnchor, LevelsAtAnchor,
    VolState,
};
use finstack_quant_models::factor::{
    FactorCovarianceMatrix, FactorModelConfig, MatchingConfig, PricingMode,
};
use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
use finstack_quant_valuations::instruments::{Attributes, Bond};
use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
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
        level_fit_quality: vec![],
        spread_duration: 1.0,
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
        schema: finstack_quant_models::factor::credit::hierarchy::CreditFactorModelSchema::CURRENT,
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
        panel_frequency:
            finstack_quant_models::factor::credit::calibration::PanelFrequency::Monthly,
        use_returns_or_levels:
            finstack_quant_models::factor::credit::calibration::PanelSpace::Returns,
        bucket_weighting:
            finstack_quant_models::factor::credit::calibration::BucketWeighting::Equal,
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
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05_f64).expect("valid rate fixture"),
        create_date(2024, Month::January, 1).unwrap(),
        create_date(2034, Month::January, 1).unwrap(),
        finstack_quant_core::dates::StubKind::ShortFront,
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

fn make_market_state(disc: DiscountCurve, haz: HazardCurve) -> MarketContextState {
    MarketContextState {
        schema_version: finstack_quant_core::wire::SchemaVersion::CURRENT,
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
        &[(1, 110.0), (3, 110.0), (5, 110.0), (10, 110.0)],
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
            target_currency: None,
            execution_policy: None,
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

/// Invariant 3 (audit fix M2): the credit leg is `Σ_lines SourceLine.credit_part
/// plus w × pull_to_par` where lines = coupon_income + roll_down and `w = s/(r+s)`
/// is the credit share (§7.4). The share `w` is not on the wire, so this test pins
/// it jointly with the rates leg: the two pull_to_par shares must be
/// complementary, each in [0, 1] of pull_to_par, and the full partition must
/// close on `carry_detail.total` — the exact identity the pre-fix code violated
/// by dropping pull_to_par from both legs (the gap was pull_to_par itself, ~99%
/// of carry on the canonical fixture).
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
    let pull_to_par = detail
        .pull_to_par
        .as_ref()
        .map(|m| m.amount())
        .unwrap_or(0.0);

    // The credit leg beyond its source-line parts must be exactly the credit
    // share of pull_to_par: a value between 0 and the full pull_to_par
    // (same sign), never more.
    let credit_ptp_share = cc.credit_carry_total.amount() - (coupon_credit + roll_credit);
    let share_fraction = if pull_to_par.abs() > TOL {
        credit_ptp_share / pull_to_par
    } else {
        0.0
    };
    assert!(
        (-TOL..=1.0 + TOL).contains(&share_fraction),
        "credit share of pull_to_par must lie in [0, 1]: share={share_fraction}, \
         credit_ptp_share={credit_ptp_share}, pull_to_par={pull_to_par}"
    );
    assert!(
        share_fraction > TOL,
        "credit leg must receive a nonzero pull_to_par share on a credit-risky \
         fixture (pre-M2 regression: pull_to_par dropped from the partition); \
         share={share_fraction}"
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

/// Invariant 5 (audit fix M2): the rates leg is `Σ_lines SourceLine.rates_part
/// plus (1 − w) × pull_to_par minus funding_cost` (§7.4), and the two legs
/// together partition the FULL carry — `rates_carry_total + credit_carry_total`
/// equals `carry_detail.total`. The partition identity is the load-bearing one;
/// the pre-fix code failed it by exactly `pull_to_par`.
#[test]
fn rates_carry_total_matches_gross_rates_source_lines() {
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
    let pull_to_par = detail
        .pull_to_par
        .as_ref()
        .map(|m| m.amount())
        .unwrap_or(0.0);

    // The two pull_to_par shares must be complementary…
    let credit_ptp_share = cc.credit_carry_total.amount() - (coupon_credit + roll_credit);
    let rates_ptp_share = cc.rates_carry_total.amount() - (coupon_rates + roll_rates);
    assert!(
        (credit_ptp_share + rates_ptp_share - pull_to_par).abs() < TOL,
        "pull_to_par shares must be complementary: credit={credit_ptp_share}, \
         rates={rates_ptp_share}, pull_to_par={pull_to_par}"
    );

    // …and the two legs must partition the full carry (the M2 identity).
    let total = detail.total.amount();
    let partition = cc.rates_carry_total.amount() + cc.credit_carry_total.amount();
    assert!(
        (partition - total).abs() < TOL,
        "carry partition failed: rates+credit={partition}, carry_detail.total={total}"
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

/// A bare `Money` value is not a valid source line.
#[test]
fn bare_money_carry_detail_is_rejected() {
    use finstack_quant_attribution::{CarryDetail, SourceLine};
    use finstack_quant_core::dates::create_date;

    let mut attr = PnlAttribution::new(
        Money::new(100.0, Currency::USD).expect("valid money fixture"),
        "STRICT-SOURCE-LINE",
        create_date(2025, time::Month::January, 1).unwrap(),
        create_date(2025, time::Month::January, 2).unwrap(),
        AttributionMethod::Parallel,
    );
    attr.carry = Money::new(30.0, Currency::USD).expect("valid money fixture");
    attr.carry_detail = Some(CarryDetail {
        total: Money::new(30.0, Currency::USD).expect("valid money fixture"),
        coupon_income: Some(SourceLine::scalar(
            Money::new(25.0, Currency::USD).expect("valid money fixture"),
        )),
        pull_to_par: None,
        roll_down: Some(SourceLine::scalar(
            Money::new(5.0, Currency::USD).expect("valid money fixture"),
        )),
        funding_cost: None,
    });

    // Serialize then replace the typed source lines with invalid bare Money.
    let mut value = serde_json::to_value(&attr).expect("serialize");
    if let Some(carry) = value
        .get_mut("carry_detail")
        .and_then(|cd| cd.as_object_mut())
    {
        for key in ["coupon_income", "roll_down"] {
            if let Some(line) = carry.get(key).cloned() {
                if let Some(total) = line.get("total").cloned() {
                    carry.insert(key.to_string(), total);
                }
            }
        }
    }
    let invalid_json = serde_json::to_string(&value).expect("re-serialize");
    serde_json::from_str::<PnlAttribution>(&invalid_json)
        .expect_err("bare Money source lines must be rejected");
}
