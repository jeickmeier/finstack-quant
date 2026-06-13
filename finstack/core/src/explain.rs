#![warn(missing_docs)]

//! Opt-in explainability infrastructure for financial computations.
//!
//! This module provides types for capturing detailed execution traces of complex
//! operations like calibration, pricing, and waterfall calculations. All tracing
//! is **opt-in** and has zero overhead when disabled.
//!
//! # Design Principles
//!
//! 1. **Opt-in by default**: Explanation is disabled unless explicitly requested
//! 2. **Size caps**: Traces are limited to prevent memory bloat
//! 3. **Stable format**: JSON-serializable with strict field names
//! 4. **Zero overhead**: Default paths have no performance impact
//!
//! # Example
//!
//! ```
//! use finstack_core::explain::{ExplainOpts, ExplanationTrace, TraceEntry};
//!
//! // Create opts with explanation enabled
//! let opts = ExplainOpts::enabled();
//!
//! // Build trace during computation
//! let mut trace = if opts.enabled {
//!     Some(ExplanationTrace::new("calibration"))
//! } else {
//!     None
//! };
//!
//! if let Some(ref mut t) = trace {
//!     t.push(TraceEntry::CalibrationIteration {
//!         iteration: 0,
//!         residual: 0.005,
//!         knots_updated: vec!["2025-01-15".to_string()],
//!         converged: false,
//!     }, opts.max_entries);
//! }
//! ```

use serde::{Deserialize, Serialize};

/// Opt-in configuration for generating explanation traces.
///
/// Controls whether detailed execution traces are captured during computation.
/// When disabled, there is zero runtime overhead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplainOpts {
    /// Whether explanation tracing is enabled
    pub enabled: bool,
    /// Maximum number of trace entries (caps memory usage)
    pub max_entries: Option<usize>,
}

impl ExplainOpts {
    /// Create options with explanation enabled and default limits.
    ///
    /// Default limit is 1000 entries to prevent unbounded memory growth.
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            max_entries: Some(1000),
        }
    }

    /// Create options with explanation disabled (zero overhead).
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            max_entries: None,
        }
    }

    /// Create options with a custom entry limit.
    pub fn with_max_entries(max_entries: usize) -> Self {
        Self {
            enabled: true,
            max_entries: Some(max_entries),
        }
    }
}

impl Default for ExplainOpts {
    /// Default is disabled (zero overhead).
    fn default() -> Self {
        Self::disabled()
    }
}

/// Container for detailed execution traces of financial computations.
///
/// Traces are organized by type (calibration, pricing, waterfall) and contain
/// a sequence of domain-specific entries. Traces can be serialized to JSON for
/// inspection, debugging, or audit purposes.
///
/// Mutation is intentionally single-threaded through `&mut self`. If multiple
/// workers need to append to one trace, wrap it in external synchronization and
/// keep ordering semantics explicit at the call site.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ExplanationTrace {
    /// Type of trace (e.g., "calibration", "pricing", "waterfall")
    #[serde(rename = "type")]
    pub trace_type: String,

    /// Sequence of trace entries
    pub entries: Vec<TraceEntry>,

    /// Whether the trace was truncated due to size limits
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated: Option<bool>,
}

impl ExplanationTrace {
    /// Create a new empty trace of the given type.
    pub fn new(trace_type: impl Into<String>) -> Self {
        Self {
            trace_type: trace_type.into(),
            entries: Vec::new(),
            truncated: None,
        }
    }

    /// Add an entry to the trace, respecting size limits.
    ///
    /// If `max_entries` is reached, marks the trace as truncated.
    pub fn push(&mut self, entry: TraceEntry, max_entries: Option<usize>) {
        if let Some(max) = max_entries {
            if self.entries.len() < max {
                self.entries.push(entry);
            } else if self.truncated.is_none() {
                self.truncated = Some(true);
            }
        } else {
            self.entries.push(entry);
        }
    }

    /// Check if the trace was truncated.
    pub fn is_truncated(&self) -> bool {
        self.truncated.unwrap_or(false)
    }

    /// Serialize to pretty-printed JSON.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// Domain-specific trace entry types.
///
/// Each variant captures relevant details for different types of computations:
/// - Calibration: iteration details, convergence status
/// - Pricing: cashflow-level PV breakdowns
/// - Waterfall: step-by-step payment allocations
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind")]
pub enum TraceEntry {
    /// Calibration solver iteration details
    #[serde(rename = "calibration_iteration")]
    CalibrationIteration {
        /// Iteration number (0-based)
        iteration: usize,
        /// Objective function residual
        residual: f64,
        /// Knot points that were updated
        knots_updated: Vec<String>,
        /// Whether convergence was achieved
        converged: bool,
    },

    /// Cashflow present value breakdown
    #[serde(rename = "cashflow_pv")]
    CashflowPV {
        /// Cashflow payment date (ISO8601)
        date: String,
        /// Cashflow amount (stored as f64 for JSON simplicity)
        cashflow_amount: f64,
        /// Cashflow currency
        cashflow_currency: String,
        /// Discount factor applied
        discount_factor: f64,
        /// Present value of this cashflow
        pv_amount: f64,
        /// PV currency
        pv_currency: String,
        /// Discount curve ID used
        curve_id: String,
    },

    /// Structured credit waterfall step
    #[serde(rename = "waterfall_step")]
    WaterfallStep {
        /// Period index
        period: usize,
        /// Step name/description
        step_name: String,
        /// Cash inflow amount
        cash_in_amount: f64,
        /// Cash inflow currency
        cash_in_currency: String,
        /// Cash outflow amount
        cash_out_amount: f64,
        /// Cash outflow currency
        cash_out_currency: String,
        /// Shortfall amount if any
        #[serde(skip_serializing_if = "Option::is_none")]
        shortfall_amount: Option<f64>,
        /// Shortfall currency
        #[serde(skip_serializing_if = "Option::is_none")]
        shortfall_currency: Option<String>,
    },

    /// Generic computation step (extensible)
    #[serde(rename = "computation_step")]
    ComputationStep {
        /// Step name
        name: String,
        /// Step description
        description: String,
        /// Arbitrary metadata (JSON object)
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<serde_json::Value>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_serialization() {
        let mut trace = ExplanationTrace::new("calibration");
        trace.push(
            TraceEntry::CalibrationIteration {
                iteration: 0,
                residual: 0.005,
                knots_updated: vec!["2025-01-15".to_string()],
                converged: false,
            },
            None,
        );

        let json = trace
            .to_json_pretty()
            .expect("JSON serialization should succeed in test");
        assert!(json.contains("\"type\": \"calibration\""));
        assert!(json.contains("\"kind\": \"calibration_iteration\""));

        // Roundtrip
        let deserialized: ExplanationTrace =
            serde_json::from_str(&json).expect("JSON deserialization should succeed in test");
        assert_eq!(deserialized.trace_type, "calibration");
        assert_eq!(deserialized.entries.len(), 1);
    }

    #[test]
    fn test_cashflow_pv_entry() {
        let entry = TraceEntry::CashflowPV {
            date: "2025-01-15".to_string(),
            cashflow_amount: 50000.0,
            cashflow_currency: "USD".to_string(),
            discount_factor: 0.95,
            pv_amount: 47500.0,
            pv_currency: "USD".to_string(),
            curve_id: "USD_GOVT".to_string(),
        };

        let json =
            serde_json::to_string(&entry).expect("JSON serialization should succeed in test");
        assert!(json.contains("\"kind\":\"cashflow_pv\""));
        assert!(json.contains("\"curve_id\":\"USD_GOVT\""));
    }
}
