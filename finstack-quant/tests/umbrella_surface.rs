//! The umbrella crate must expose every domain crate so downstream users can
//! reach the full API through a single dependency.
//!
//! Regression: attribution was omitted entirely, which made portfolio and
//! scenario APIs naming attribution types unusable through the umbrella.

fn assert_type<T>() {
    let _ = std::mem::size_of::<T>();
}

/// Attribution types named in public portfolio and scenario signatures must be
/// reachable through the umbrella.
#[test]
fn umbrella_exposes_attribution_types_used_by_portfolio_and_scenarios() {
    assert_type::<finstack_quant::attribution::AttributionMethod>();
    assert_type::<finstack_quant::attribution::PnlAttribution>();
    assert_type::<finstack_quant::attribution::AttributionFactor>();
}

/// The umbrella must alias every domain crate.
#[test]
fn umbrella_exposes_every_domain_module() {
    assert_type::<finstack_quant::core::currency::Currency>();
    assert_type::<finstack_quant::attribution::AttributionMethod>();
}
