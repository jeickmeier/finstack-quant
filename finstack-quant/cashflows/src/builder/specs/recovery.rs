//! Recovery model specifications for credit instruments.

use super::vector_at;

/// Recovery model specification.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RecoveryModelSpec {
    /// Recovery rate as fraction (0.0 to 1.0, e.g., 0.40 for 40%)
    pub rate: f64,
    /// Recovery lag in months
    pub recovery_lag: u32,
    /// Loss severity (`1 − recovery`) by month of default as decimals in
    /// `[0, 1]`, month 1 first; the last value is held. When present it
    /// replaces `rate` for defaults in that month.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity_vector: Option<Vec<f64>>,
}

impl RecoveryModelSpec {
    /// Standard recovery with lag.
    ///
    /// # Arguments
    ///
    /// * `rate` - Recovery rate as a decimal share in `[0.0, 1.0]`.
    /// * `recovery_lag` - Number of months between default and recovery cashflow.
    ///
    /// # Returns
    ///
    /// Recovery model with the supplied rate and lag. Call
    /// [`validate`](Self::validate) for range checking.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::RecoveryModelSpec;
    ///
    /// let spec = RecoveryModelSpec::with_lag(0.40, 12);
    /// spec.validate()?;
    /// assert_eq!(spec.recovery_lag, 12);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    ///
    /// # References
    ///
    /// - `docs/REFERENCES.md#isda-cds-standard-model`
    pub fn with_lag(rate: f64, recovery_lag: u32) -> Self {
        Self {
            rate,
            recovery_lag,
            severity_vector: None,
        }
    }

    /// Attach a loss-severity vector by month of default.
    ///
    /// # Arguments
    ///
    /// * `severity_vector` - Loss severity (`1 − recovery`) per seasoning
    ///   month of the default as decimals in `[0, 1]`, month 1 first; the
    ///   last value is held. Must be non-empty.
    ///
    /// # Returns
    ///
    /// The model with the vector attached; `rate` stays as the documented
    /// flat fallback but no longer drives recoveries.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::RecoveryModelSpec;
    ///
    /// let spec = RecoveryModelSpec::with_lag(0.40, 12).with_severity_vector(vec![0.7, 0.6]);
    /// assert!((spec.recovery_rate(1) - 0.3).abs() < 1e-15);
    /// assert!((spec.recovery_rate(24) - 0.4).abs() < 1e-15);
    /// ```
    pub fn with_severity_vector(mut self, severity_vector: Vec<f64>) -> Self {
        self.severity_vector = Some(severity_vector);
        self
    }

    /// Recovery rate for a default in the given seasoning month.
    ///
    /// # Arguments
    ///
    /// * `seasoning_months` - Months since origination of the defaulting
    ///   month; month 0 reads the first vector entry.
    ///
    /// # Returns
    ///
    /// `1 − severity` from `severity_vector` when present (last value held),
    /// otherwise `rate`. Values are clamped to `[0, 1]`.
    pub fn recovery_rate(&self, seasoning_months: u32) -> f64 {
        match &self.severity_vector {
            Some(severities) if !severities.is_empty() => {
                let severity = vector_at(severities, seasoning_months, "severity_vector")
                    .unwrap_or(1.0 - self.rate);
                (1.0 - severity).clamp(0.0, 1.0)
            }
            _ => self.rate,
        }
    }

    /// Validate the recovery model parameters.
    ///
    /// # Errors
    ///
    /// Returns `Validation` error if:
    /// - `rate` is not in `[0.0, 1.0]`
    /// - `rate` is NaN or infinite
    /// - `severity_vector` is present but empty or holds a value outside
    ///   `[0.0, 1.0]`
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if !self.rate.is_finite() || !(0.0..=1.0).contains(&self.rate) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "RecoveryModelSpec rate ({}) must be in [0.0, 1.0] and finite",
                self.rate
            )));
        }
        if let Some(severities) = &self.severity_vector {
            if severities.is_empty() {
                return Err(finstack_quant_core::Error::Validation(
                    "RecoveryModelSpec severity_vector must not be empty".to_string(),
                ));
            }
            for (index, severity) in severities.iter().enumerate() {
                if !severity.is_finite() || !(0.0..=1.0).contains(severity) {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "RecoveryModelSpec severity_vector[{index}] ({severity}) must be in [0.0, 1.0] and finite"
                    )));
                }
            }
        }
        Ok(())
    }
}
