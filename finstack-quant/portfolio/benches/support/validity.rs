//! Output contracts for successful portfolio benchmark cases.

use criterion::Bencher;
use finstack_quant_portfolio::Portfolio;
use finstack_quant_portfolio::valuation::PortfolioValuation;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::STANDARD;
use std::hint::black_box;

/// Check a selected case once outside timing, then consume each measured result.
///
/// # Arguments
///
/// * `bencher` - Criterion bencher for the selected case.
/// * `operation` - Operation that requires success on every invocation.
/// * `validate` - Output contract checked only during untimed preflight.
pub fn bench<T>(
    bencher: &mut Bencher<'_>,
    mut operation: impl FnMut() -> T,
    validate: impl FnOnce(&T),
) {
    let output = operation();
    validate(&output);
    drop(output);
    bencher.iter(|| black_box(operation()));
}

/// Require complete finite values and every applicable metric from the menu.
///
/// # Arguments
///
/// * `portfolio` - Source book whose position IDs and base currency must survive valuation.
/// * `valuation` - Successful output to inspect outside the timing loop.
/// * `menu` - Exact risk menu requested by the benchmark; empty for PV-only cases.
pub fn valuation(portfolio: &Portfolio, valuation: &PortfolioValuation, menu: &[MetricId]) {
    assert_eq!(valuation.position_values.len(), portfolio.positions().len());
    assert_eq!(valuation.as_of, portfolio.as_of);
    assert_eq!(
        valuation.total_base_currency.currency(),
        portfolio.base_currency
    );
    assert!(valuation.total_base_currency.amount().is_finite());
    assert!(valuation.degraded_positions.is_empty());
    for position in portfolio.positions() {
        let value = valuation
            .get_position_value(position.position_id.as_str())
            .expect("every benchmark position must be valued");
        assert!(value.value_native.amount().is_finite());
        assert!(value.value_base.amount().is_finite());
        assert_eq!(value.value_base.currency(), portfolio.base_currency);
        assert!(
            value.risk_metrics_complete,
            "{}: incomplete risk",
            position.position_id
        );
        assert!(
            value.risk_error.is_none(),
            "{}: {:?}",
            position.position_id,
            value.risk_error
        );
        let result = value
            .valuation_result
            .as_ref()
            .expect("canonical pricing result");
        assert_eq!(result.instrument_id, position.instrument_id.as_str());
        assert!(result.value.amount().is_finite());
        assert_eq!(result.value.currency(), value.value_native.currency());
        for metric in position
            .instrument
            .applicable_metrics(menu, STANDARD.metrics())
        {
            assert!(
                result
                    .measures
                    .get(&metric)
                    .is_some_and(|value| value.is_finite()),
                "{}: missing or non-finite {metric}",
                position.position_id
            );
        }
        assert!(result.measures.values().all(|value| value.is_finite()));
    }
    assert!(valuation.by_entity.values().all(|value| {
        value.currency() == portfolio.base_currency && value.amount().is_finite()
    }));
}
