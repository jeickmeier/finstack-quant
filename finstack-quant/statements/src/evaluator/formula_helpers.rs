//! Statistical and data-collection helpers for formula evaluation.
//!
//! Pure functions that operate on evaluation contexts to gather, sort, and
//! reduce historical node values. These have no dependency on the expression
//! evaluator, so they can be shared by both `formula.rs` and
//! `formula_aggregates.rs` without creating cycles.

use crate::error::Result;
use crate::evaluator::context::EvaluationContext;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::math::{
    mean_or_nan, median_or_nan, sample_std_or_nan, sample_variance_or_nan,
};
use std::collections::BTreeMap;
use std::rc::Rc;

/// Coerce a numeric value into DSL boolean semantics.
///
/// Non-zero finite values are truthy; zero and non-finite values are falsey.
#[inline]
pub(crate) fn is_truthy(value: f64) -> bool {
    value.is_finite() && value != 0.0
}

/// Decode an internal `__cs__<component>__<instrument>` reference.
pub(crate) fn decode_cs_reference(name: &str) -> Option<(&str, &str)> {
    let rest = name.strip_prefix("__cs__")?;
    let (component, instrument) = rest.split_once("__")?;
    if component.is_empty() || instrument.is_empty() || instrument.contains("__") {
        return None;
    }
    Some((component, instrument))
}

/// Get a single historical value for a node or cs-reference at a target period.
pub(crate) fn get_historical_column_value(
    context: &EvaluationContext,
    node_name: &str,
    target_period: &PeriodId,
) -> Option<f64> {
    if let Some((component, instrument_or_total)) = decode_cs_reference(node_name) {
        context
            .get_historical_cs_value(component, instrument_or_total, target_period)
            .ok()
    } else {
        context.get_historical_value(node_name, target_period)
    }
}

/// Collect historical values sorted chronologically.
///
/// Returns an `Rc<BTreeMap<PeriodId, f64>>` containing all historical periods
/// plus the current period's value. The result is memoized on the
/// `EvaluationContext`, so repeated calls within the same period are O(1)
/// refcount bumps instead of rebuilding the map. Used by rolling-window,
/// expanding, and statistical helpers; cache lifetime is the surrounding
/// period's evaluation.
pub(crate) fn collect_historical_values_sorted(
    node_name: &str,
    context: &EvaluationContext,
) -> Result<Rc<BTreeMap<PeriodId, f64>>> {
    if let Some(cached) = context
        .sorted_history_cache
        .borrow()
        .get(node_name)
        .cloned()
    {
        return Ok(cached);
    }

    // Only periods at or before the context's evaluation period are visible.
    // Lagged evaluation builds contexts for past periods over the full
    // historical map, so without this filter nested aggregates (e.g.
    // `lag(rolling_mean(x, 4), 2)`) would window data after the lagged
    // evaluation point — a silent look-ahead bias.
    let sorted_periods =
        if let Some((component, instrument_or_total)) = decode_cs_reference(node_name) {
            let mut sorted_periods = BTreeMap::new();
            for period in context.historical_capital_structure_cashflows.keys() {
                if *period > context.period_id {
                    continue;
                }
                if let Ok(value) =
                    context.get_historical_cs_value(component, instrument_or_total, period)
                {
                    sorted_periods.insert(*period, value);
                }
            }
            if let Ok(current) = context.get_cs_value(component, instrument_or_total) {
                sorted_periods.insert(context.period_id, current);
            }
            sorted_periods
        } else {
            let mut sorted_periods = BTreeMap::new();
            for (period, values) in context.historical_results.iter() {
                if *period > context.period_id {
                    continue;
                }
                if let Some(value) = values.get(node_name) {
                    sorted_periods.insert(*period, *value);
                }
            }
            if let Ok(current) = context.get_value(node_name) {
                sorted_periods.insert(context.period_id, current);
            }
            sorted_periods
        };

    let result = Rc::new(sorted_periods);
    context
        .sorted_history_cache
        .borrow_mut()
        .insert(node_name.to_string(), Rc::clone(&result));
    Ok(result)
}

/// Collect values for a rolling window in chronological order.
/// Returns values from oldest to newest within the window.
pub(crate) fn collect_rolling_window_values(
    node_name: &str,
    context: &EvaluationContext,
    window_size: usize,
) -> Result<Vec<f64>> {
    if window_size == 0 {
        return Ok(Vec::new());
    }

    let sorted = collect_historical_values_sorted(node_name, context)?;

    // Take the last `window_size` values directly in chronological order — avoids
    // the rev/take/reverse round-trip the previous implementation incurred per call.
    let skip_count = sorted.len().saturating_sub(window_size);
    Ok(sorted.values().skip(skip_count).copied().collect())
}

/// Collect all historical values for a node including current.
pub(crate) fn collect_all_historical_values(
    node_name: &str,
    context: &EvaluationContext,
) -> Result<Vec<f64>> {
    let sorted = collect_historical_values_sorted(node_name, context)?;
    Ok(sorted.values().copied().collect())
}

/// Collect values for a node over a closed period range [start, end].
///
/// Periods are compared using their natural ordering. Values are returned in
/// chronological order (oldest -> newest).
pub(crate) fn collect_period_range_values(
    node_name: &str,
    context: &EvaluationContext,
    start: PeriodId,
    end: PeriodId,
) -> Result<Vec<f64>> {
    let sorted = collect_historical_values_sorted(node_name, context)?;
    Ok(sorted
        .iter()
        .filter(|(period, _)| **period >= start && **period <= end)
        .map(|(_, value)| *value)
        .collect())
}

/// Calculate mean of values.
pub(crate) fn calculate_mean(values: &[f64]) -> Result<f64> {
    Ok(mean_or_nan(values))
}

/// Calculate standard deviation of values.
///
/// Uses sample standard deviation (sqrt of sample variance) per financial industry standards.
pub(crate) fn calculate_std(values: &[f64]) -> Result<f64> {
    Ok(sample_std_or_nan(values))
}

/// Calculate variance of values.
///
/// Uses sample variance (Bessel's correction with n-1 denominator) per financial industry standards.
/// This is the unbiased estimator required by Bloomberg, Excel VAR.S(), pandas.var(ddof=1), etc.
pub(crate) fn calculate_variance(values: &[f64]) -> Result<f64> {
    Ok(sample_variance_or_nan(values))
}

/// Calculate median of values.
pub(crate) fn calculate_median(values: &[f64]) -> Result<f64> {
    Ok(median_or_nan(values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::NodeId;
    use indexmap::IndexMap;
    use std::sync::Arc;

    fn make_context_with_history(
        node: &str,
        history: &[(PeriodId, f64)],
        current_period: PeriodId,
    ) -> EvaluationContext {
        let mut node_to_column = IndexMap::new();
        node_to_column.insert(NodeId::new(node), 0);

        let mut historical = IndexMap::new();
        for (period, value) in history {
            let mut entries = IndexMap::new();
            entries.insert(node.to_string(), *value);
            historical.insert(*period, entries);
        }

        EvaluationContext::new(
            current_period,
            Arc::new(node_to_column),
            Arc::new(historical),
        )
    }

    /// Populating the cache, mutating the current period via `set_value`, and
    /// re-reading must reflect the new value — not the value snapshotted into
    /// the cache before mutation. Guards against future regressions to the
    /// invalidation hook in `EvaluationContext::set_value`.
    #[test]
    fn cache_invalidates_on_set_value_within_period() {
        let p1 = PeriodId::quarter(2025, 1);
        let current = PeriodId::quarter(2025, 2);
        let mut ctx = make_context_with_history("revenue", &[(p1, 100.0)], current);

        ctx.set_value("revenue", 110.0).expect("set initial");

        // Populate cache.
        let first = collect_historical_values_sorted("revenue", &ctx).expect("first lookup");
        assert_eq!(first.get(&current), Some(&110.0));

        // Mutate; cache must be invalidated.
        ctx.set_value("revenue", 125.0).expect("set updated");

        let second = collect_historical_values_sorted("revenue", &ctx).expect("second lookup");
        assert_eq!(
            second.get(&current),
            Some(&125.0),
            "stale cache returned old value after set_value mutation"
        );
        // The historical period must still be present and unchanged.
        assert_eq!(second.get(&p1), Some(&100.0));
    }

    /// Cloning a context with a populated cache must not cause one side's
    /// later mutation to corrupt the other side's view. The `Rc<BTreeMap>`
    /// values are immutable through `Rc`, so independent invalidations on
    /// each clone keep their own cache state distinct.
    #[test]
    fn cache_clone_independence() {
        let p1 = PeriodId::quarter(2025, 1);
        let current = PeriodId::quarter(2025, 2);
        let mut original = make_context_with_history("revenue", &[(p1, 100.0)], current);
        original.set_value("revenue", 110.0).expect("set original");

        // Populate the original's cache.
        let from_original =
            collect_historical_values_sorted("revenue", &original).expect("original lookup");
        assert_eq!(from_original.get(&current), Some(&110.0));

        // Clone the context — should deep-clone the IndexMap shell while
        // sharing the `Rc<BTreeMap>` values. Mutating the clone must not
        // change what the original's cache returns.
        let mut cloned = original.clone();
        cloned.set_value("revenue", 999.0).expect("set clone");

        let from_original_after =
            collect_historical_values_sorted("revenue", &original).expect("re-read original");
        assert_eq!(
            from_original_after.get(&current),
            Some(&110.0),
            "original cache was corrupted by clone mutation"
        );

        let from_clone =
            collect_historical_values_sorted("revenue", &cloned).expect("clone lookup");
        assert_eq!(
            from_clone.get(&current),
            Some(&999.0),
            "clone did not see its own mutation"
        );
    }
}
