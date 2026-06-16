//! Netting set identification and instrument margin results.

use super::simm_types::SimmSensitivities;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;

/// Identifies a margin netting set.
///
/// Instruments in the same netting set can offset each other for margin
/// calculation purposes. The netting set is typically defined by the
/// CSA agreement (bilateral) or by CCP membership (cleared) — these two
/// shapes are mutually exclusive, so the type encodes them as enum
/// variants rather than as a struct with two `Option<String>` fields
/// that could in principle both be set or both be unset.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum NettingSetId {
    /// A bilateral netting set scoped by counterparty + CSA.
    Bilateral {
        /// Counterparty identifier
        counterparty_id: String,
        /// CSA identifier
        csa_id: String,
    },
    /// A cleared netting set scoped by CCP membership.
    Cleared {
        /// CCP identifier (also used as the counterparty id)
        ccp_id: String,
    },
}

impl NettingSetId {
    /// Create a bilateral netting set ID.
    #[must_use]
    pub fn bilateral(counterparty_id: impl Into<String>, csa_id: impl Into<String>) -> Self {
        NettingSetId::Bilateral {
            counterparty_id: counterparty_id.into(),
            csa_id: csa_id.into(),
        }
    }

    /// Create a cleared netting set ID.
    #[must_use]
    pub fn cleared(ccp_id: impl Into<String>) -> Self {
        NettingSetId::Cleared {
            ccp_id: ccp_id.into(),
        }
    }

    /// Check if this is a cleared netting set.
    #[must_use]
    pub fn is_cleared(&self) -> bool {
        matches!(self, NettingSetId::Cleared { .. })
    }

    /// The counterparty identifier — for cleared netting sets this is
    /// the CCP id, for bilateral it is the explicit counterparty id.
    #[must_use]
    pub fn counterparty_id(&self) -> &str {
        match self {
            NettingSetId::Bilateral {
                counterparty_id, ..
            } => counterparty_id.as_str(),
            NettingSetId::Cleared { ccp_id } => ccp_id.as_str(),
        }
    }

    /// CSA identifier, if this is a bilateral netting set.
    #[must_use]
    pub fn csa_id(&self) -> Option<&str> {
        match self {
            NettingSetId::Bilateral { csa_id, .. } => Some(csa_id.as_str()),
            NettingSetId::Cleared { .. } => None,
        }
    }

    /// CCP identifier, if this is a cleared netting set.
    #[must_use]
    pub fn ccp_id(&self) -> Option<&str> {
        match self {
            NettingSetId::Cleared { ccp_id } => Some(ccp_id.as_str()),
            NettingSetId::Bilateral { .. } => None,
        }
    }
}

impl std::fmt::Display for NettingSetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NettingSetId::Cleared { ccp_id } => write!(f, "CCP:{ccp_id}"),
            NettingSetId::Bilateral {
                counterparty_id,
                csa_id,
            } => write!(f, "{counterparty_id}:{csa_id}"),
        }
    }
}

/// Result of calculating margin for an instrument.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InstrumentMarginResult {
    /// Instrument identifier
    pub instrument_id: String,
    /// Calculation date
    #[schemars(with = "String")]
    pub as_of: Date,
    /// Initial margin requirement
    pub initial_margin: Money,
    /// Variation margin requirement (can be negative = return)
    pub variation_margin: Money,
    /// Total margin requirement (IM + VM if positive)
    pub total_margin: Money,
    /// IM calculation methodology used
    pub im_methodology: crate::types::ImMethodology,
    /// Whether instrument is cleared or bilateral
    pub is_cleared: bool,
    /// Netting set identifier
    pub netting_set: Option<NettingSetId>,
    /// SIMM sensitivities (if SIMM was used)
    pub sensitivities: Option<SimmSensitivities>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_netting_set_id() {
        let bilateral = NettingSetId::bilateral("COUNTERPARTY_A", "CSA_001");
        assert!(!bilateral.is_cleared());
        assert_eq!(bilateral.to_string(), "COUNTERPARTY_A:CSA_001");

        let cleared = NettingSetId::cleared("LCH");
        assert!(cleared.is_cleared());
        assert_eq!(cleared.to_string(), "CCP:LCH");
    }

    #[test]
    fn variant_accessors_return_correct_field_per_variant() {
        let bi = NettingSetId::bilateral("ACME", "CSA-1");
        assert_eq!(bi.counterparty_id(), "ACME");
        assert_eq!(bi.csa_id(), Some("CSA-1"));
        assert_eq!(bi.ccp_id(), None);

        let cl = NettingSetId::cleared("LCH");
        assert_eq!(cl.counterparty_id(), "LCH");
        assert_eq!(cl.csa_id(), None);
        assert_eq!(cl.ccp_id(), Some("LCH"));
    }

    #[test]
    fn json_serialization_uses_kind_tag() {
        // Bilateral: tag = "bilateral", fields inline.
        let bi = NettingSetId::bilateral("ACME", "CSA-1");
        let json = serde_json::to_value(&bi).expect("serialize bilateral");
        assert_eq!(
            json,
            serde_json::json!({
                "kind": "bilateral",
                "counterparty_id": "ACME",
                "csa_id": "CSA-1",
            }),
            "bilateral serialization shape"
        );

        // Cleared: tag = "cleared", fields inline.
        let cl = NettingSetId::cleared("LCH");
        let json = serde_json::to_value(&cl).expect("serialize cleared");
        assert_eq!(
            json,
            serde_json::json!({
                "kind": "cleared",
                "ccp_id": "LCH",
            }),
            "cleared serialization shape"
        );
    }

    #[test]
    fn json_roundtrip_preserves_variant_and_fields() {
        for original in [
            NettingSetId::bilateral("CPTY-1", "CSA-001"),
            NettingSetId::cleared("LCH"),
            NettingSetId::bilateral("multi-word counterparty", "CSA with spaces"),
        ] {
            let serialized = serde_json::to_string(&original).expect("serialize");
            let recovered: NettingSetId = serde_json::from_str(&serialized).expect("deserialize");
            assert_eq!(recovered, original, "round-trip for {original:?}");
        }
    }

    #[test]
    fn json_deserialization_rejects_missing_tag() {
        // Old struct shape (`{counterparty_id, csa_id, ccp_id}`) without
        // a `kind` discriminator must fail with a clear serde error so a
        // pre-migration value cannot be silently misclassified.
        let old_shape = serde_json::json!({
            "counterparty_id": "ACME",
            "csa_id": "CSA-1",
            "ccp_id": null,
        });
        let result: std::result::Result<NettingSetId, _> = serde_json::from_value(old_shape);
        assert!(
            result.is_err(),
            "old struct shape (no `kind` tag) must be rejected, got {result:?}"
        );
    }
}
