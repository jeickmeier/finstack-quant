use super::operations::*;
use super::types::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::hierarchy::HierarchyTarget;
use finstack_quant_core::math::Compounding;
use finstack_quant_valuations::pricer::InstrumentType;
use indexmap::IndexMap;

/// One value of every [`OperationSpec`] variant.
fn one_of_each() -> Vec<OperationSpec> {
    let target = HierarchyTarget {
        path: vec!["Rates".into()],
        tag_filter: None,
    };
    let attrs: IndexMap<String, String> = [("sector".to_string(), "energy".to_string())]
        .into_iter()
        .collect();
    let nodes = vec![("5Y".to_string(), 10.0)];
    vec![
        OperationSpec::MarketFxPct {
            base: Currency::EUR,
            quote: Currency::USD,
            pct: 1.0,
        },
        OperationSpec::EquityPricePct {
            ids: vec!["SPX".into()],
            pct: 1.0,
        },
        OperationSpec::InstrumentPricePctByAttr {
            attrs: attrs.clone(),
            pct: 1.0,
        },
        OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD-OIS".into(),
            discount_curve_id: None,
            bp: 1.0,
        },
        OperationSpec::CurveNodeBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD-OIS".into(),
            discount_curve_id: None,
            nodes: nodes.clone(),
            match_mode: TenorMatchMode::Exact,
        },
        OperationSpec::VolIndexParallelPts {
            curve_id: "VIX".into(),
            points: 1.0,
        },
        OperationSpec::VolIndexNodePts {
            curve_id: "VIX".into(),
            nodes,
            match_mode: TenorMatchMode::Exact,
        },
        OperationSpec::BaseCorrParallelPts {
            surface_id: "CDX".into(),
            points: 0.01,
        },
        OperationSpec::BaseCorrBucketPts {
            surface_id: "CDX".into(),
            detachment_bp: None,
            points: 0.01,
        },
        OperationSpec::VolSurfaceParallelPct {
            vol_surface_id: "SPX".into(),
            pct: 1.0,
        },
        OperationSpec::VolSurfaceBucketPct {
            vol_surface_id: "SPX".into(),
            tenors: None,
            strikes: None,
            pct: 1.0,
        },
        OperationSpec::StmtForecastPercent {
            node_id: "Revenue".into(),
            pct: 1.0,
        },
        OperationSpec::StmtForecastAssign {
            node_id: "Revenue".into(),
            value: 1.0,
        },
        OperationSpec::RateBinding {
            binding: RateBindingSpec {
                node_id: "Rate".into(),
                curve_id: "USD-OIS".into(),
                tenor: "1Y".into(),
                compounding: Compounding::Periodic(std::num::NonZeroU32::MIN),
                day_count: None,
            },
        },
        OperationSpec::InstrumentSpreadBpByAttr { attrs, bp: 1.0 },
        OperationSpec::InstrumentPricePctByType {
            instrument_types: vec![InstrumentType::Bond],
            pct: 1.0,
        },
        OperationSpec::InstrumentSpreadBpByType {
            instrument_types: vec![InstrumentType::Bond],
            bp: 1.0,
        },
        OperationSpec::AssetCorrelationPts { delta_pts: 0.01 },
        OperationSpec::PrepayDefaultCorrelationPts { delta_pts: 0.01 },
        OperationSpec::HierarchyCurveParallelBp {
            curve_kind: CurveKind::Discount,
            target: target.clone(),
            bp: 1.0,
            discount_curve_id: None,
        },
        OperationSpec::HierarchyVolSurfaceParallelPct {
            target: target.clone(),
            pct: 1.0,
        },
        OperationSpec::HierarchyEquityPricePct {
            target: target.clone(),
            pct: 1.0,
        },
        OperationSpec::HierarchyBaseCorrParallelPts {
            target,
            points: 0.01,
        },
        OperationSpec::time_roll_forward("1M", None, None),
    ]
}

/// `kind()` is the serde `kind` tag for every variant, so hosts and
/// diagnostics can name an operation without serializing it.
#[test]
fn kind_is_the_serde_tag_for_every_variant() {
    let operations = one_of_each();
    assert_eq!(operations.len(), 24);
    for operation in operations {
        let wire = serde_json::to_value(&operation).expect("operation serializes");
        assert_eq!(Some(operation.kind()), wire["kind"].as_str(), "{wire}");
    }
}
