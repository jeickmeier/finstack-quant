//! Market parameter types for instrument pricing.

use finstack_quant_core::types::{CurveId, Percentage};
#[cfg(feature = "ts_export")]
use ts_rs::TS;

use serde::{Deserialize, Serialize};

/// Option type for pricing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
pub enum OptionType {
    /// Call option
    Call,
    /// Put option
    Put,
}

impl std::fmt::Display for OptionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptionType::Call => write!(f, "call"),
            OptionType::Put => write!(f, "put"),
        }
    }
}

impl std::str::FromStr for OptionType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "call" | "buy" | "buy_protection" => Ok(OptionType::Call),
            "put" | "sell" | "sell_protection" => Ok(OptionType::Put),
            other => Err(format!("Unknown option type: {}", other)),
        }
    }
}

/// Exercise style for options
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ExerciseStyle {
    /// European exercise (only at expiry)
    #[default]
    European,
    /// American exercise (any time before/at expiry)
    American,
    /// Bermudan exercise (specific dates before expiry)
    Bermudan,
}

impl std::fmt::Display for ExerciseStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExerciseStyle::European => write!(f, "european"),
            ExerciseStyle::American => write!(f, "american"),
            ExerciseStyle::Bermudan => write!(f, "bermudan"),
        }
    }
}

impl std::str::FromStr for ExerciseStyle {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "european" => Ok(ExerciseStyle::European),
            "american" => Ok(ExerciseStyle::American),
            "bermudan" => Ok(ExerciseStyle::Bermudan),
            other => Err(format!("Unknown exercise style: {}", other)),
        }
    }
}

/// Settlement type for options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SettlementType {
    /// Physical delivery
    Physical,
    /// Cash settlement
    Cash,
}

impl std::fmt::Display for SettlementType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettlementType::Physical => write!(f, "physical"),
            SettlementType::Cash => write!(f, "cash"),
        }
    }
}

impl std::str::FromStr for SettlementType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "physical" => Ok(SettlementType::Physical),
            "cash" => Ok(SettlementType::Cash),
            other => Err(format!("Unknown settlement type: {}", other)),
        }
    }
}

/// Credit parameters for CDS instruments
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreditParams {
    /// Reference entity (issuer being protected)
    pub reference_entity: String,
    /// Recovery rate (0.0 to 1.0)
    pub recovery_rate: f64,
    /// Credit curve identifier
    pub credit_curve_id: CurveId,
}

impl CreditParams {
    /// Create new credit parameters.
    ///
    /// Recovery-rate validation is provided separately by
    /// [`CreditParams::validate`] so this constructor's signature stays
    /// infallible. Callers that need the `recovery_rate ∈ [0.0, 1.0)`
    /// invariant enforced — consistently with `ProtectionLegSpec` — should
    /// call `validate`.
    pub fn new(
        reference_entity: impl Into<String>,
        recovery_rate: f64,
        credit_curve_id: impl Into<CurveId>,
    ) -> Self {
        Self {
            reference_entity: reference_entity.into(),
            recovery_rate,
            credit_curve_id: credit_curve_id.into(),
        }
    }

    /// Create new credit parameters using typed percentage recovery.
    pub fn new_pct(
        reference_entity: impl Into<String>,
        recovery_rate: Percentage,
        credit_curve_id: impl Into<CurveId>,
    ) -> Self {
        Self {
            reference_entity: reference_entity.into(),
            recovery_rate: recovery_rate.as_decimal(),
            credit_curve_id: credit_curve_id.into(),
        }
    }

    /// Standard corporate credit with 40% recovery
    pub fn corporate_standard(
        reference_entity: impl Into<String>,
        credit_curve_id: impl Into<CurveId>,
    ) -> Self {
        Self::new(reference_entity, 0.40, credit_curve_id)
    }

    /// Sovereign credit with 30% recovery
    pub fn sovereign_standard(
        reference_entity: impl Into<String>,
        credit_curve_id: impl Into<CurveId>,
    ) -> Self {
        Self::new(reference_entity, 0.30, credit_curve_id)
    }

    /// Validate that the recovery rate is within valid bounds `[0.0, 1.0)`.
    ///
    /// Delegates to the shared internal recovery-rate validator — the same one
    /// used by `ProtectionLegSpec::new` — so credit instruments enforce a
    /// single, consistent recovery-rate invariant.
    ///
    /// `CreditParams::new` does not call this (the struct also has public
    /// fields and is built by serde), which is why the audit flagged
    /// `CreditParams` as inconsistent with `ProtectionLegSpec::new`. Call
    /// `validate` after construction to close that gap.
    ///
    /// # Errors
    /// Returns an error stating the attempted value and the required range when
    /// `recovery_rate` is not finite or lies outside `[0.0, 1.0)`.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        crate::instruments::common_impl::validation::validate_recovery_rate(self.recovery_rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_parsing_display_and_position_sign_cover_aliases() {
        assert_eq!(OptionType::Call.to_string(), "call");
        assert_eq!("buy".parse::<OptionType>(), Ok(OptionType::Call));
        assert_eq!("sell_protection".parse::<OptionType>(), Ok(OptionType::Put));
        assert!("weird".parse::<OptionType>().is_err());

        assert_eq!(ExerciseStyle::default(), ExerciseStyle::European);
        assert_eq!(
            "american".parse::<ExerciseStyle>(),
            Ok(ExerciseStyle::American)
        );
        assert_eq!(ExerciseStyle::Bermudan.to_string(), "bermudan");
        assert!("odd".parse::<ExerciseStyle>().is_err());

        assert_eq!(SettlementType::Cash.to_string(), "cash");
        assert_eq!(
            "physical".parse::<SettlementType>(),
            Ok(SettlementType::Physical)
        );
        assert!("gross".parse::<SettlementType>().is_err());
    }

    #[test]
    fn credit_typed_constructors_preserve_typed_inputs() {
        let credit = CreditParams::new_pct("ACME", Percentage::new(35.0), "ACME-CDS");
        assert_eq!(credit.reference_entity, "ACME");
        assert!((credit.recovery_rate - 0.35).abs() < 1e-12);
        assert_eq!(credit.credit_curve_id.as_str(), "ACME-CDS");

        let corp = CreditParams::corporate_standard("CORP", "CORP-CDS");
        let sov = CreditParams::sovereign_standard("UST", "UST-CDS");
        assert!((corp.recovery_rate - 0.40).abs() < 1e-12);
        assert!((sov.recovery_rate - 0.30).abs() < 1e-12);
    }

    #[test]
    fn credit_params_validate_enforces_recovery_rate_bounds() {
        // Failure mode: `CreditParams::new` skips `validate_recovery_rate`,
        // unlike `ProtectionLegSpec::new`. `CreditParams::validate` closes the
        // gap with the same shared validator.
        let above = CreditParams::new("ACME", 1.5, "ACME-CDS");
        let err = above
            .validate()
            .expect_err("recovery rate 1.5 must be rejected by validate");
        assert!(
            err.to_string().to_lowercase().contains("recovery rate"),
            "error should name the recovery rate: {err}"
        );
        // R = 1.0 is rejected (zero LGD degenerates protection legs), matching
        // the shared validator used by ProtectionLegSpec.
        assert!(CreditParams::new("ACME", 1.0, "ACME-CDS")
            .validate()
            .is_err());
        assert!(CreditParams::new("ACME", -0.1, "ACME-CDS")
            .validate()
            .is_err());
        assert!(CreditParams::new("ACME", f64::NAN, "ACME-CDS")
            .validate()
            .is_err());
        // Valid mid-range recovery is accepted.
        assert!(CreditParams::new("ACME", 0.4, "ACME-CDS")
            .validate()
            .is_ok());
        // The shared corporate/sovereign presets are within bounds.
        assert!(CreditParams::corporate_standard("CORP", "CORP-CDS")
            .validate()
            .is_ok());
        assert!(CreditParams::sovereign_standard("UST", "UST-CDS")
            .validate()
            .is_ok());
        // A struct-literal spec that bypassed `new` is still checkable.
        let bad = CreditParams {
            reference_entity: "X".to_string(),
            recovery_rate: 1.2,
            credit_curve_id: CurveId::new("X-CDS"),
        };
        assert!(bad.validate().is_err());
    }

    #[test]
    fn credit_constructor_and_serde_roundtrip_preserve_defaults() {
        let credit = CreditParams::new("Issuer", 0.4, "ISSUER-CDS");
        assert_eq!(credit.recovery_rate, 0.4);
        let credit_json = serde_json::to_string(&credit);
        assert!(credit_json.is_ok());
        if let Ok(json) = credit_json {
            let roundtrip = serde_json::from_str::<CreditParams>(&json);
            assert!(roundtrip.is_ok());
            if let Ok(back) = roundtrip {
                assert_eq!(back.reference_entity, "Issuer");
                assert_eq!(back.credit_curve_id.as_str(), "ISSUER-CDS");
            }
        }
    }
}
