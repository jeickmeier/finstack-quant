//! Integration tests for explainability infrastructure.

use finstack_quant_core::explain::{ExplainOpts, ExplanationTrace, TraceEntry};

#[test]
fn test_explain_opts_default_is_disabled() {
    let opts = ExplainOpts::default();
    assert!(!opts.enabled);
    assert!(opts.max_entries.is_none());
}

#[test]
fn test_explain_opts_enabled() {
    let opts = ExplainOpts::enabled();
    assert!(opts.enabled);
    assert_eq!(opts.max_entries, Some(1000));
}

#[test]
fn test_explanation_trace_size_cap() {
    let mut trace = ExplanationTrace::new("calibration");
    let opts = ExplainOpts::with_max_entries(3);

    // Add more entries than the cap
    for i in 0..10 {
        trace.push(
            TraceEntry::CalibrationIteration {
                iteration: i,
                residual: 0.001 * (i as f64),
                knots_updated: vec![format!("{}y", i)],
                converged: false,
            },
            opts.max_entries,
        );
    }

    // Should be capped at 3 entries
    assert_eq!(trace.entries.len(), 3);
    assert!(trace.is_truncated());
}

#[test]
fn test_explanation_trace_serialization() {
    let mut trace = ExplanationTrace::new("pricing");
    trace.push(
        TraceEntry::CashflowPV {
            date: "2025-01-15".to_string(),
            cashflow_amount: 50000.0,
            cashflow_currency: "USD".to_string(),
            discount_factor: 0.95,
            pv_amount: 47500.0,
            pv_currency: "USD".to_string(),
            curve_id: "USD_GOVT".to_string(),
        },
        None,
    );

    // Serialize to JSON
    let json = trace.to_json_pretty().expect("Failed to serialize");

    // Verify structure
    assert!(json.contains("\"type\": \"pricing\""));
    assert!(json.contains("\"kind\": \"cashflow_pv\""));
    assert!(json.contains("USD_GOVT"));

    // Roundtrip test
    let deserialized: ExplanationTrace =
        serde_json::from_str(&json).expect("Failed to deserialize");
    assert_eq!(deserialized.trace_type, "pricing");
    assert_eq!(deserialized.entries.len(), 1);
}

#[test]
fn test_waterfall_trace_entry() {
    let entry = TraceEntry::WaterfallStep {
        period: 1,
        step_name: "Senior Tranche Interest".to_string(),
        cash_in_amount: 100000.0,
        cash_in_currency: "USD".to_string(),
        cash_out_amount: 95000.0,
        cash_out_currency: "USD".to_string(),
        shortfall_amount: Some(5000.0),
        shortfall_currency: Some("USD".to_string()),
    };

    let json = serde_json::to_string(&entry).expect("Failed to serialize");
    assert!(json.contains("\"kind\":\"waterfall_step\""));
    assert!(json.contains("Senior Tranche Interest"));
}

#[test]
fn test_computation_step_extensibility() {
    let mut trace = ExplanationTrace::new("custom");
    trace.push(
        TraceEntry::ComputationStep {
            name: "Risk Calculation".to_string(),
            description: "Computing DV01 via bump and revalue".to_string(),
            metadata: Some(serde_json::json!({
                "bump_size_bp": 1.0,
                "parallel_shift": true
            })),
        },
        None,
    );

    assert_eq!(trace.entries.len(), 1);
    let json = trace.to_json_pretty().expect("Failed to serialize");
    assert!(json.contains("Risk Calculation"));
    assert!(json.contains("bump_size_bp"));
}

#[test]
fn test_zero_overhead_when_disabled() {
    // When explanation is disabled, we should not create traces
    let opts = ExplainOpts::disabled();
    assert!(!opts.enabled);

    // This pattern should be used in production code:
    let trace: Option<ExplanationTrace> = if opts.enabled {
        Some(ExplanationTrace::new("test"))
    } else {
        None
    };

    assert!(trace.is_none());
}

#[cfg(test)]
mod property_tests {
    use super::*;

    #[test]
    fn property_trace_entries_never_exceed_max() {
        let max = 5;
        let opts = ExplainOpts::with_max_entries(max);
        let mut trace = ExplanationTrace::new("test");

        // Try to add many entries
        for i in 0..100 {
            trace.push(
                TraceEntry::CalibrationIteration {
                    iteration: i,
                    residual: 0.001,
                    knots_updated: vec![],
                    converged: false,
                },
                opts.max_entries,
            );
        }

        // Should never exceed max
        assert!(trace.entries.len() <= max);
        assert!(trace.is_truncated());
    }
}
