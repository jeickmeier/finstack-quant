use serde::{Deserialize, Serialize};

/// Covenant check result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CovenantReport {
    /// Type of covenant being checked
    pub covenant_type: String,

    /// Stable machine-readable covenant instance identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub covenant_id: Option<String>,

    /// Whether the covenant passed
    pub passed: bool,

    /// Actual value of the metric
    pub actual_value: Option<f64>,

    /// Required threshold
    pub threshold: Option<f64>,

    /// Details or explanation
    pub details: Option<String>,

    /// Cushion relative to threshold (positive => passing buffer)
    pub headroom: Option<f64>,
}

impl CovenantReport {
    /// Create a passing covenant report.
    pub fn passed(covenant_type: &str) -> Self {
        Self {
            covenant_type: covenant_type.to_string(),
            covenant_id: None,
            passed: true,
            actual_value: None,
            threshold: None,
            details: None,
            headroom: None,
        }
    }

    /// Create a failing covenant report.
    pub fn failed(covenant_type: &str) -> Self {
        Self {
            covenant_type: covenant_type.to_string(),
            covenant_id: None,
            passed: false,
            actual_value: None,
            threshold: None,
            details: None,
            headroom: None,
        }
    }

    /// Attach the stable covenant identifier.
    pub fn with_covenant_id(mut self, id: &str) -> Self {
        self.covenant_id = Some(id.to_string());
        self
    }

    /// Add actual value to the report.
    pub fn with_actual(mut self, value: f64) -> Self {
        self.actual_value = Some(value);
        self
    }

    /// Add threshold to the report.
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = Some(threshold);
        self
    }

    /// Add details to the report.
    pub fn with_details(mut self, details: &str) -> Self {
        self.details = Some(details.to_string());
        self
    }

    /// Attach headroom (positive = cushion, negative = deficit).
    pub fn with_headroom(mut self, headroom: f64) -> Self {
        self.headroom = Some(headroom);
        self
    }
}
