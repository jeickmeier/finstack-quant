//! Versioned serde contract and generated JSON Schema for margin payloads.

use serde::{Deserialize, Serialize};
#[cfg(feature = "json-schema")]
use serde_json::Value;

#[cfg(feature = "json-schema")]
use crate::{
    calculators::CcpMethodology, metrics::ExcessCollateral, metrics::Haircut01,
    metrics::MarginFundingCost, metrics::MarginUtilization, regulatory::EadResult,
    regulatory::FrtbRiskClass, regulatory::FrtbSbaResult, regulatory::FrtbSensitivities,
    regulatory::SaCcrNettingSetConfig, regulatory::SaCcrTrade, types::SimmSensitivitiesJson,
    xva::mva::ImDecayProfile, xva::mva::MvaResult, xva::types::ExposureProfile,
    xva::types::FundingConfig, xva::types::XvaResult, ImCollateralResult, ImResult, RepoMarginSpec,
    ScheduleAssetClass, SimmSensitivities, SimmVersion, VmResult,
};
use crate::{CsaSpec, MarginCall, OtcMarginSpec};

/// Stable base URI for margin-owned schemas.
pub const MARGIN_SCHEMA_BASE: &str = "https://finstack_quant.dev/schemas/margin/1/";
/// Filename of the published margin schema.
pub const MARGIN_SCHEMA_FILENAME: &str = "margin.schema.json";
/// Canonical title of the published margin schema.
pub const MARGIN_SCHEMA_TITLE: &str = "Finstack Quant Margin Specification";
/// Canonical description of the published margin schema.
pub const MARGIN_SCHEMA_DESCRIPTION: &str = "OTC derivative margin specifications including CSA terms, thresholds, and collateral eligibility. Covers ISDA CSA, BCBS-IOSCO regulatory margin, and CCP clearing requirements.";
/// Required marker for the published margin contract.
pub const MARGIN_SCHEMA: &str = "finstack_quant.margin/1";

/// Typed value of the required margin schema marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum MarginSchema {
    /// The sole pre-release margin contract.
    #[serde(rename = "finstack_quant.margin/1")]
    Margin,
}

impl MarginSchema {
    /// The exact marker required by every persisted margin envelope.
    pub const CURRENT: Self = Self::Margin;
}

/// Strict root envelope for every supported margin payload.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields, untagged)]
pub enum MarginEnvelope {
    /// An OTC margin specification.
    OtcMarginSpec {
        /// Required contract marker.
        schema: MarginSchema,
        /// OTC margin specification payload.
        otc_margin_spec: OtcMarginSpec,
    },
    /// A standalone CSA specification.
    CsaSpec {
        /// Required contract marker.
        schema: MarginSchema,
        /// CSA specification payload.
        csa_spec: CsaSpec,
    },
    /// A concrete margin call.
    MarginCall {
        /// Required contract marker.
        schema: MarginSchema,
        /// Margin call payload.
        margin_call: MarginCall,
    },
}

impl MarginEnvelope {
    /// Wrap an OTC margin specification in the canonical envelope.
    ///
    /// # Arguments
    ///
    /// * `otc_margin_spec` - Complete OTC margin specification to persist.
    #[must_use]
    pub fn otc_margin_spec(otc_margin_spec: OtcMarginSpec) -> Self {
        Self::OtcMarginSpec {
            schema: MarginSchema::CURRENT,
            otc_margin_spec,
        }
    }

    /// Wrap a CSA specification in the canonical envelope.
    ///
    /// # Arguments
    ///
    /// * `csa_spec` - Complete CSA specification to persist.
    #[must_use]
    pub fn csa_spec(csa_spec: CsaSpec) -> Self {
        Self::CsaSpec {
            schema: MarginSchema::CURRENT,
            csa_spec,
        }
    }

    /// Wrap a margin call in the canonical envelope.
    ///
    /// # Arguments
    ///
    /// * `margin_call` - Complete margin call to persist.
    #[must_use]
    pub fn margin_call(margin_call: MarginCall) -> Self {
        Self::MarginCall {
            schema: MarginSchema::CURRENT,
            margin_call,
        }
    }

    /// Deserialize a strict margin envelope from JSON bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Complete UTF-8 JSON document containing one margin envelope.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] for malformed JSON,
    /// an absent or unsupported schema marker, an unknown field, or a payload
    /// that does not match exactly one supported envelope variant.
    pub fn from_slice(bytes: &[u8]) -> finstack_quant_core::Result<Self> {
        serde_json::from_slice(bytes).map_err(|error| {
            finstack_quant_core::Error::Validation(format!(
                "invalid {MARGIN_SCHEMA} envelope: {error}"
            ))
        })
    }
}

/// A canonical CSA specification.
///
/// The `csa_spec` branch is the one a caller authors most often; the VM
/// parameters, eligible-collateral schedule and call timing come from their
/// documented defaults rather than invented numbers.
#[cfg(feature = "json-schema")]
fn margin_examples() -> finstack_quant_core::Result<Vec<Value>> {
    let registry = crate::registry::embedded_registry()?;
    let csa = crate::types::CsaSpec {
        id: "CSA-ACME-2024".to_string(),
        base_currency: finstack_quant_core::currency::Currency::USD,
        calendar_id: "nyse".to_string(),
        vm_params: crate::types::VmParameters::regulatory_standard(
            finstack_quant_core::currency::Currency::USD,
        )?,
        im_params: None,
        eligible_collateral: Default::default(),
        call_timing: registry.defaults.timing.standard.clone(),
        collateral_curve_id: "USD-OIS".into(),
    };
    let envelope = MarginEnvelope::csa_spec(csa);
    let value = serde_json::to_value(&envelope).map_err(|error| {
        finstack_quant_core::Error::Internal(format!("serialize margin example: {error}"))
    })?;
    Ok(vec![value])
}

/// The crate's complete schema registry.
///
/// This lives in the library, not the generator binary, so the generator, the
/// contract tests and the bindings all render from one definition. Rendering
/// goes through [`finstack_quant_core::schema::SchemaArtifact::generate`].
#[cfg(feature = "json-schema")]
pub const ARTIFACTS: &[finstack_quant_core::schema::SchemaArtifact] = &[
    finstack_quant_core::schema::SchemaArtifact::new::<MarginEnvelope>(
        "schemas/margin/1/margin.schema.json",
        "https://finstack_quant.dev/schemas/margin/1/margin.schema.json",
        MARGIN_SCHEMA_TITLE,
        MARGIN_SCHEMA_DESCRIPTION,
    )
    .with_kind(finstack_quant_core::schema::SchemaKind::Input)
    .with_summary(
        "One of three closed root shapes: an OTC margin spec, a CSA spec, or a margin call.",
    )
    .with_examples(margin_examples),
    finstack_quant_core::schema_artifact!(
        CcpMethodology,
        "margin",
        "ccp_methodology",
        Component,
        "CCP methodology type."
    ),
    finstack_quant_core::schema_artifact!(
        EadResult,
        "margin",
        "ead_result",
        Output,
        "SA-CCR Exposure at Default result."
    ),
    finstack_quant_core::schema_artifact!(
        ExcessCollateral,
        "margin",
        "excess_collateral",
        Output,
        "Excess collateral result."
    ),
    finstack_quant_core::schema_artifact!(
        ExposureProfile,
        "margin",
        "exposure_profile",
        Input,
        "Exposure profile computed at each time grid point."
    ),
    finstack_quant_core::schema_artifact!(
        FrtbRiskClass,
        "margin",
        "frtb_risk_class",
        Component,
        "FRTB risk classes per BCBS d457."
    ),
    finstack_quant_core::schema_artifact!(
        FrtbSbaResult,
        "margin",
        "frtb_sba_result",
        Output,
        "Complete FRTB SBA capital charge result."
    ),
    finstack_quant_core::schema_artifact!(
        FrtbSensitivities,
        "margin",
        "frtb_sensitivities",
        Input,
        "FRTB sensitivity inputs organized by risk class."
    ),
    finstack_quant_core::schema_artifact!(
        FundingConfig,
        "margin",
        "funding_config",
        Input,
        "Funding cost/benefit configuration for FVA and MVA calculation."
    ),
    finstack_quant_core::schema_artifact!(
        Haircut01,
        "margin",
        "haircut01",
        Output,
        "Haircut sensitivity (Haircut01) result."
    ),
    finstack_quant_core::schema_artifact!(
        ImCollateralResult,
        "margin",
        "im_collateral_result",
        Output,
        "One-way IM collateral account after applying the CSA's allocated threshold."
    ),
    finstack_quant_core::schema_artifact!(
        ImDecayProfile,
        "margin",
        "im_decay_profile",
        Input,
        "Deterministic decay applied to today's SIMM IM to approximate `E[IM(t)]`."
    ),
    finstack_quant_core::schema_artifact!(
        ImResult,
        "margin",
        "im_result",
        Output,
        "Initial margin calculation result."
    ),
    finstack_quant_core::schema_artifact!(
        MarginFundingCost,
        "margin",
        "margin_funding_cost",
        Output,
        "Margin funding cost result."
    ),
    finstack_quant_core::schema_artifact!(
        MarginUtilization,
        "margin",
        "margin_utilization",
        Output,
        "Margin utilization result."
    ),
    finstack_quant_core::schema_artifact!(
        MvaResult,
        "margin",
        "mva_result",
        Output,
        "Result of an MVA computation."
    ),
    finstack_quant_core::schema_artifact!(
        RepoMarginSpec,
        "margin",
        "repo_margin_spec",
        Input,
        "GMRA 2011 compliant repo margin specification."
    ),
    finstack_quant_core::schema_artifact!(
        SaCcrNettingSetConfig,
        "margin",
        "sa_ccr_netting_set_config",
        Input,
        "Netting set configuration for SA-CCR."
    ),
    finstack_quant_core::schema_artifact!(
        SaCcrTrade,
        "margin",
        "sa_ccr_trade",
        Input,
        "A single derivative trade for SA-CCR EAD computation."
    ),
    finstack_quant_core::schema_artifact!(
        ScheduleAssetClass,
        "margin",
        "schedule_asset_class",
        Component,
        "Asset class for schedule-based IM calculation."
    ),
    finstack_quant_core::schema_artifact!(
        SimmSensitivities,
        "margin",
        "simm_sensitivities",
        Input,
        "SIMM sensitivity inputs organized by risk class."
    ),
    finstack_quant_core::schema_artifact!(
        SimmSensitivitiesJson,
        "margin",
        "simm_sensitivities_json",
        Input,
        "JSON-friendly representation of `SimmSensitivities`."
    ),
    finstack_quant_core::schema_artifact!(
        SimmVersion,
        "margin",
        "simm_version",
        Component,
        "SIMM version identifier."
    ),
    finstack_quant_core::schema_artifact!(
        VmResult,
        "margin",
        "vm_result",
        Output,
        "Variation margin calculation result."
    ),
    finstack_quant_core::schema_artifact!(
        XvaResult,
        "margin",
        "xva_result",
        Output,
        "Result of XVA calculations."
    ),
];

/// Generate the published margin schema exactly as it is checked in.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Internal`] if schemars output cannot
/// be represented as a JSON object.
#[cfg(feature = "json-schema")]
pub fn generated_margin_schema() -> finstack_quant_core::Result<Value> {
    ARTIFACTS[0].generate()
}
