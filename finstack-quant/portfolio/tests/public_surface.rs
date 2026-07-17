//! Public surface tests for portfolio.

use finstack_quant_portfolio::factor_model::{
    FactorAssignmentReport, FactorModel, FactorModelBuilder, PositionChange, RiskDecomposition,
    UnmatchedEntry,
};

#[test]
fn portfolio_root_and_factor_model_module_exports_compile() {
    fn assert_type<T>() {
        let _ = std::mem::size_of::<T>();
    }

    assert_type::<FactorModel>();
    assert_type::<FactorModelBuilder>();
    assert_type::<RiskDecomposition>();
    assert_type::<FactorAssignmentReport>();
    assert_type::<PositionChange>();
    assert_type::<UnmatchedEntry>();
}

/// Attribution types appear in portfolio's public signatures. Direct portfolio
/// consumers must be able to name them without adding a version-sensitive
/// attribution dependency of their own.
#[test]
fn portfolio_reexports_attribution_types_in_its_public_api() {
    fn assert_type<T>() {
        let _ = std::mem::size_of::<T>();
    }

    assert_type::<finstack_quant_portfolio::attribution::AttributionMethod>();
    assert_type::<finstack_quant_portfolio::attribution::PnlAttribution>();
    assert_type::<finstack_quant_portfolio::attribution::RatesCurvesAttribution>();
    assert_type::<finstack_quant_portfolio::attribution::FxAttribution>();
    assert_type::<finstack_quant_portfolio::attribution::TaylorAttributionConfig>();
    assert_type::<finstack_quant_portfolio::attribution::ExecutionPolicy>();
}
