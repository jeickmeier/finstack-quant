//! Credit leg and issuer credit parameters shared by credit derivatives.

use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::types::CurveId;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Named contractual date regime of a CDS product.
///
/// The regime owns how the premium grid, its stub and its endpoints are
/// derived; no regime is inferred and none is a default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CdsDateRegime {
    /// Standard quarterly CDS roll dates (20 March, June, September and
    /// December): the premium grid runs quarterly from an accrual start on a
    /// roll date, with a short final period when the maturity is off the
    /// grid. The leg's `frequency` must be quarterly; its `stub` is not used.
    Standard,
    /// A bespoke schedule from the leg's own `frequency`, `stub` and
    /// boundaries.
    Bespoke,
}

impl std::fmt::Display for CdsDateRegime {
    /// Writes the serde label (`standard`, `bespoke`).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Standard => "standard",
            Self::Bespoke => "bespoke",
        })
    }
}

impl std::str::FromStr for CdsDateRegime {
    type Err = finstack_quant_core::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "standard" => Ok(Self::Standard),
            "bespoke" => Ok(Self::Bespoke),
            other => Err(finstack_quant_core::Error::Validation(format!(
                "unknown CDS date regime '{other}'; expected 'standard' or 'bespoke'"
            ))),
        }
    }
}

/// Specification for CDS premium legs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PremiumLegSpec {
    /// Contractual date regime of the premium schedule: `standard` CDS roll
    /// dates or a `bespoke` schedule from `frequency` and `stub`.
    pub date_regime: CdsDateRegime,
    /// Start date of protection.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub start: Date,
    /// End date of protection.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub end: Date,
    /// Payment frequency.
    pub frequency: Tenor,
    /// Stub convention of a bespoke schedule; the standard regime owns its
    /// own stub.
    #[serde(default = "finstack_quant_valuations_core::serde_defaults::stub_short_front")]
    pub stub: StubKind,
    /// Business day convention.
    #[serde(default = "finstack_quant_valuations_core::serde_defaults::bdc_modified_following")]
    pub business_day_convention: BusinessDayConvention,
    /// Holiday calendar identifier.
    pub calendar_id: Option<finstack_quant_core::types::CalendarId>,
    /// Day count convention.
    pub day_count: DayCount,
    /// Contractual running coupon in basis points (e.g., `100` = 1% per
    /// annum, the standard CDX.NA.IG coupon; `500` for high yield).
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub coupon_bp: Decimal,
    /// Discount curve identifier.
    pub discount_curve_id: CurveId,
}

/// Specification for CDS protection legs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProtectionLegSpec {
    /// Hazard curve identifier for default probabilities.
    pub credit_curve_id: CurveId,
    /// Recovery rate as a decimal fraction in `[0.0, 1.0)`.
    pub recovery_rate: f64,
    /// Settlement delay in business days.
    pub settlement_delay: u16,
}

impl ProtectionLegSpec {
    /// Create a new protection leg specification with validation.
    ///
    /// # Arguments
    /// * `credit_curve_id` - Identifier for the hazard/credit curve.
    /// * `recovery_rate` - Recovery rate as a decimal fraction in `[0.0, 1.0)`
    ///   (e.g., 0.4 = 40%).
    /// * `settlement_delay` - Settlement delay in business days.
    ///
    /// # Errors
    /// Returns an error if `recovery_rate` is outside `[0.0, 1.0)`.
    pub fn new(
        credit_curve_id: impl Into<CurveId>,
        recovery_rate: f64,
        settlement_delay: u16,
    ) -> finstack_quant_core::Result<Self> {
        let spec = Self {
            credit_curve_id: credit_curve_id.into(),
            recovery_rate,
            settlement_delay,
        };
        spec.validate()?;
        Ok(spec)
    }

    /// Validate that `recovery_rate` lies within `[0.0, 1.0)`.
    ///
    /// Useful when the spec was built from its public fields or via serde rather than
    /// [`ProtectionLegSpec::new`].
    ///
    /// # Errors
    /// Returns an error if `recovery_rate` is non-finite or outside `[0.0, 1.0)`.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        finstack_quant_core::validation::validate_recovery_rate(self.recovery_rate)
    }
}

/// Issuer/reference entity credit parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreditParams {
    /// Reference entity (issuer being protected).
    pub reference_entity: String,
    /// Recovery rate (`0.0` to `1.0`).
    pub recovery_rate: f64,
    /// Credit curve identifier.
    pub credit_curve_id: CurveId,
}

impl CreditParams {
    /// Create new credit parameters.
    ///
    /// Call [`CreditParams::validate`] after construction when the `recovery_rate ∈ [0.0, 1.0)`
    /// invariant must be enforced on externally supplied parameters.
    ///
    /// # Arguments
    ///
    /// * `reference_entity` - Issuer or reference entity name being protected.
    /// * `recovery_rate` - Expected recovery fraction on default (`0.40` = 40%).
    /// * `credit_curve_id` - Identifier of the hazard/survival curve in market data.
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

    /// Standard corporate credit with 40% recovery.
    ///
    /// # Arguments
    ///
    /// * `reference_entity` - Corporate issuer or reference entity name.
    /// * `credit_curve_id` - Identifier of the hazard/survival curve in market data.
    pub fn corporate_standard(
        reference_entity: impl Into<String>,
        credit_curve_id: impl Into<CurveId>,
    ) -> Self {
        Self::new(reference_entity, 0.40, credit_curve_id)
    }

    /// Validate that the recovery rate is within valid bounds `[0.0, 1.0)`.
    ///
    /// # Errors
    /// Returns an error stating the attempted value and the required range when
    /// `recovery_rate` is not finite or lies outside `[0.0, 1.0)`.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        finstack_quant_core::validation::validate_recovery_rate(self.recovery_rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credit_typed_constructors_preserve_typed_inputs() {
        let corp = CreditParams::corporate_standard("CORP", "CORP-CDS");
        assert!((corp.recovery_rate - 0.40).abs() < 1e-12);
    }

    #[test]
    fn credit_params_validate_enforces_recovery_rate_bounds() {
        let above = CreditParams::new("ACME", 1.5, "ACME-CDS");
        let err = above
            .validate()
            .expect_err("recovery rate 1.5 must be rejected by validate");
        assert!(
            err.to_string().to_lowercase().contains("recovery rate"),
            "error should name the recovery rate: {err}"
        );
        assert!(
            CreditParams::new("ACME", 1.0, "ACME-CDS")
                .validate()
                .is_err()
        );
        assert!(
            CreditParams::new("ACME", -0.1, "ACME-CDS")
                .validate()
                .is_err()
        );
        assert!(
            CreditParams::new("ACME", f64::NAN, "ACME-CDS")
                .validate()
                .is_err()
        );
        assert!(
            CreditParams::new("ACME", 0.4, "ACME-CDS")
                .validate()
                .is_ok()
        );
        assert!(
            CreditParams::corporate_standard("CORP", "CORP-CDS")
                .validate()
                .is_ok()
        );

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
        let json = serde_json::to_string(&credit).expect("serialize");
        let back: CreditParams = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.reference_entity, "Issuer");
        assert_eq!(back.credit_curve_id.as_str(), "ISSUER-CDS");
    }

    #[test]
    fn protection_leg_spec_validate_checks_recovery_bounds() {
        let invalid = ProtectionLegSpec {
            credit_curve_id: CurveId::new("ACME-CDS"),
            recovery_rate: 1.0,
            settlement_delay: 2,
        };
        assert!(invalid.validate().is_err());
        let valid = ProtectionLegSpec::new("ACME-CDS", 0.4, 2).expect("valid leg");
        assert!(valid.validate().is_ok());
    }
}
