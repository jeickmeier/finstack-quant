use finstack_core::currency::Currency;
use finstack_core::market_data::hierarchy::ResolutionMode;
use finstack_scenarios::{
    Compounding, CurveKind, HierarchyTarget, InstrumentType, OperationSpec, RateBindingSpec,
    ScenarioSpec, TenorMatchMode, TimeRollMode, VolSurfaceKind,
};
#[test]
fn scenario_validate_rejects_empty_id() {
    let scenario = ScenarioSpec {
        id: "   ".into(),
        name: None,
        description: None,
        operations: vec![],
        priority: 0,
        resolution_mode: ResolutionMode::default(),
    };

    let error = scenario
        .validate()
        .expect_err("blank scenario IDs should fail validation");
    let message = error.to_string();

    assert!(message.contains("Scenario ID cannot be empty"));
}

#[test]
fn scenario_validate_rejects_multiple_time_rolls() {
    let scenario = ScenarioSpec {
        id: "two_rolls".into(),
        name: None,
        description: None,
        operations: vec![
            OperationSpec::TimeRollForward {
                period: "1D".into(),
                apply_shocks: true,
                roll_mode: TimeRollMode::BusinessDays,
            },
            OperationSpec::TimeRollForward {
                period: "1W".into(),
                apply_shocks: false,
                roll_mode: TimeRollMode::CalendarDays,
            },
        ],
        priority: 0,
        resolution_mode: ResolutionMode::default(),
    };

    let error = scenario
        .validate()
        .expect_err("multiple time rolls should fail validation");
    let message = error.to_string();

    assert!(message.contains("at most one is allowed"));
    assert!(message.contains("2"));
}

#[test]
fn scenario_validate_prefixes_invalid_operation_index() {
    let scenario = ScenarioSpec {
        id: "bad_op".into(),
        name: None,
        description: None,
        operations: vec![
            OperationSpec::EquityPricePct {
                ids: vec!["SPY".into()],
                pct: -5.0,
            },
            OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Discount,
                curve_id: "   ".into(),
                discount_curve_id: None,
                bp: 10.0,
            },
        ],
        priority: 0,
        resolution_mode: ResolutionMode::default(),
    };

    let error = scenario
        .validate()
        .expect_err("invalid operation should bubble up with its index");
    let message = error.to_string();

    assert!(message.contains("Operation 1"));
    assert!(message.contains("curve_id"));
}

#[test]
fn operation_validate_rejects_invalid_inputs() {
    let same_ccy = OperationSpec::MarketFxPct {
        base: Currency::USD,
        quote: Currency::USD,
        pct: 5.0,
    };
    // FX still rejects -100% — driving spot to zero would NaN-propagate.
    // Equity / instrument / vol shocks at -100% are now accepted as legitimate
    // tail-risk stress.
    let pct_floor = OperationSpec::MarketFxPct {
        base: Currency::EUR,
        quote: Currency::USD,
        pct: -100.0,
    };
    let empty_hierarchy = OperationSpec::HierarchyCurveParallelBp {
        curve_kind: CurveKind::Discount,
        target: HierarchyTarget {
            path: vec![],
            tag_filter: None,
        },
        bp: 25.0,
        discount_curve_id: None,
    };

    assert!(same_ccy
        .validate()
        .expect_err("same-currency FX pair should fail")
        .to_string()
        .contains("Base and quote currencies must be different"));
    assert!(pct_floor
        .validate()
        .expect_err("percent floor should fail")
        .to_string()
        .contains("greater than -100%"));
    assert!(empty_hierarchy
        .validate()
        .expect_err("empty hierarchy target should fail")
        .to_string()
        .contains("Hierarchy target path cannot be empty"));
}

/// W7 regression: correlation point deltas must lie in [-2, 2]. Anything
/// outside cannot produce a valid correlation (|Δρ| ≤ 2 since ρ ∈ [-1, 1])
/// and is almost certainly a unit mistake (e.g. 25 meaning "0.25").
#[test]
fn operation_validate_rejects_out_of_range_correlation_deltas() {
    let asset_corr_large = OperationSpec::AssetCorrelationPts { delta_pts: 25.0 };
    let asset_corr_negative = OperationSpec::AssetCorrelationPts { delta_pts: -3.5 };
    let base_corr_parallel = OperationSpec::BaseCorrParallelPts {
        surface_id: "CDX.IG".into(),
        points: 5.0,
    };

    for op in [asset_corr_large, asset_corr_negative, base_corr_parallel] {
        let err = op
            .validate()
            .expect_err("out-of-range correlation delta must be rejected");
        assert!(
            err.to_string().contains("[-2, 2] points"),
            "missing bound message: {err}"
        );
    }

    // In-range values still pass.
    OperationSpec::AssetCorrelationPts { delta_pts: 0.15 }
        .validate()
        .expect("0.15 is a sane correlation delta");
    OperationSpec::BaseCorrParallelPts {
        surface_id: "CDX.IG".into(),
        points: -0.05,
    }
    .validate()
    .expect("-0.05 is a sane base-correlation point shock");
}

#[test]
fn operation_validate_rejects_curve_node_and_binding_shape_errors() {
    let empty_nodes = OperationSpec::CurveNodeBp {
        curve_kind: CurveKind::Discount,
        curve_id: "USD_SOFR".into(),
        discount_curve_id: None,
        nodes: vec![],
        match_mode: TenorMatchMode::Interpolate,
    };
    let empty_tenor = OperationSpec::CurveNodeBp {
        curve_kind: CurveKind::Discount,
        curve_id: "USD_SOFR".into(),
        discount_curve_id: Some("USD_OIS".into()),
        nodes: vec![("   ".into(), 5.0)],
        match_mode: TenorMatchMode::Interpolate,
    };
    let bad_binding = OperationSpec::RateBinding {
        binding: RateBindingSpec {
            node_id: "InterestRate".into(),
            curve_id: "USD_SOFR".into(),
            tenor: " ".into(),
            compounding: Compounding::Continuous,
            day_count: None,
        },
    };
    let blank_roll = OperationSpec::TimeRollForward {
        period: " ".into(),
        apply_shocks: true,
        roll_mode: TimeRollMode::BusinessDays,
    };

    assert!(empty_nodes
        .validate()
        .expect_err("empty curve nodes should fail")
        .to_string()
        .contains("Curve nodes cannot be empty"));
    assert!(empty_tenor
        .validate()
        .expect_err("blank curve node tenor should fail")
        .to_string()
        .contains("Curve node tenor cannot be empty"));
    assert!(bad_binding
        .validate()
        .expect_err("blank rate binding tenor should fail")
        .to_string()
        .contains("RateBinding tenor cannot be empty"));
    assert!(blank_roll
        .validate()
        .expect_err("blank time roll period should fail")
        .to_string()
        .contains("Time roll period cannot be empty"));
}

#[test]
fn operation_validate_rejects_non_finite_and_fx_floor_violation() {
    let non_finite_fx = OperationSpec::MarketFxPct {
        base: Currency::EUR,
        quote: Currency::USD,
        pct: f64::NAN,
    };
    let fx_floor = OperationSpec::MarketFxPct {
        base: Currency::EUR,
        quote: Currency::USD,
        pct: -100.0,
    };

    // Vol surfaces and instrument-by-type shocks now accept any finite pct,
    // including <= -100, because driving vol to zero or below is meaningful in
    // tail-risk stress (and `check_arbitrage` flags downstream issues). Only
    // FX retains the strict floor.
    let vol_floor_relaxed = OperationSpec::VolSurfaceBucketPct {
        surface_kind: VolSurfaceKind::Equity,
        surface_id: "SPX".into(),
        tenors: None,
        strikes: Some(vec![100.0]),
        pct: -100.0,
    };
    let type_floor_relaxed = OperationSpec::InstrumentPricePctByType {
        instrument_types: vec![InstrumentType::Bond],
        pct: -125.0,
    };

    assert!(non_finite_fx
        .validate()
        .expect_err("NaN percentages should fail")
        .to_string()
        .contains("must be finite"));
    assert!(fx_floor
        .validate()
        .expect_err("FX percent floor should fail")
        .to_string()
        .contains("greater than -100%"));
    vol_floor_relaxed
        .validate()
        .expect("vol surface -100% bucket shock should now validate");
    type_floor_relaxed
        .validate()
        .expect("instrument-type -125% shock should now validate");
}

#[test]
fn scenario_validate_accepts_mixed_valid_operations() {
    let scenario = ScenarioSpec {
        id: "broad_valid_spec".into(),
        name: Some("Broad valid spec".into()),
        description: None,
        operations: vec![
            OperationSpec::MarketFxPct {
                base: Currency::EUR,
                quote: Currency::USD,
                pct: 5.0,
            },
            OperationSpec::CurveNodeBp {
                curve_kind: CurveKind::Discount,
                curve_id: "USD_SOFR".into(),
                discount_curve_id: Some("USD_OIS".into()),
                nodes: vec![("2Y".into(), 10.0), ("10Y".into(), -5.0)],
                match_mode: TenorMatchMode::Interpolate,
            },
            OperationSpec::BaseCorrBucketPts {
                surface_id: "CDX_IG".into(),
                detachment_bps: Some(vec![300, 700]),
                maturities: Some(vec!["5Y".into()]),
                points: 0.02,
            },
            OperationSpec::VolSurfaceBucketPct {
                surface_kind: VolSurfaceKind::Equity,
                surface_id: "SPX".into(),
                tenors: Some(vec!["1Y".into()]),
                strikes: Some(vec![95.0, 105.0]),
                pct: 10.0,
            },
            OperationSpec::RateBinding {
                binding: RateBindingSpec {
                    node_id: "InterestRate".into(),
                    curve_id: "USD_SOFR".into(),
                    tenor: "1Y".to_string(),
                    compounding: Compounding::Continuous,
                    day_count: None,
                },
            },
            OperationSpec::InstrumentSpreadBpByType {
                instrument_types: vec![InstrumentType::Bond, InstrumentType::Loan],
                bp: 25.0,
            },
            OperationSpec::HierarchyVolSurfaceParallelPct {
                surface_kind: VolSurfaceKind::Equity,
                target: HierarchyTarget {
                    path: vec!["Vol".into(), "Equity".into()],
                    tag_filter: None,
                },
                pct: 7.5,
            },
            OperationSpec::HierarchyBaseCorrParallelPts {
                target: HierarchyTarget {
                    path: vec!["Credit".into(), "Index".into()],
                    tag_filter: None,
                },
                points: 0.01,
            },
            OperationSpec::TimeRollForward {
                period: "1W".into(),
                apply_shocks: true,
                roll_mode: TimeRollMode::CalendarDays,
            },
        ],
        priority: 1,
        resolution_mode: ResolutionMode::default(),
    };

    scenario
        .validate()
        .expect("mixed valid scenario should pass validation");
}

/// `ScenarioEngine::apply` must enforce `ScenarioSpec::validate` on every
/// entry path so FFI callers that bypass validate() still get safety.
#[test]
fn engine_apply_rejects_invalid_spec() {
    use finstack_core::market_data::context::MarketContext;
    use finstack_scenarios::{ExecutionContext, ScenarioEngine};
    use finstack_statements::FinancialModelSpec;
    use time::macros::date;

    let spec = ScenarioSpec {
        id: "".into(),
        name: None,
        description: None,
        operations: vec![],
        priority: 0,
        resolution_mode: ResolutionMode::default(),
    };

    let mut market = MarketContext::new();
    let mut model = FinancialModelSpec::new("test", vec![]);
    let mut ctx = ExecutionContext {
        market: &mut market,
        model: Some(&mut model),
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: date!(2025 - 01 - 15),
    };

    let engine = ScenarioEngine::new();
    let err = engine
        .apply(&spec, &mut ctx)
        .expect_err("empty-id spec should be rejected by engine.apply");
    assert!(err.to_string().contains("Scenario ID cannot be empty"));
}
