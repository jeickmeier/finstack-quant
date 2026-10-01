//! Versioned serde contract and generated JSON Schema for margin payloads.

use serde::{Deserialize, Serialize};
#[cfg(feature = "json-schema")]
use serde_json::Value;

#[cfg(feature = "json-schema")]
use crate::{
    calculators::CcpMethodology, constants::MarginConstants, metrics::ExcessCollateral,
    metrics::Haircut01, metrics::MarginFundingCost, metrics::MarginUtilization,
    regulatory::EadResult, regulatory::FrtbRiskClass, regulatory::FrtbSbaResult,
    regulatory::FrtbSensitivities, regulatory::SaCcrNettingSetConfig, regulatory::SaCcrTrade,
    types::SimmSensitivitiesJson, xva::mva::ImDecayProfile, xva::mva::MvaResult,
    xva::types::ExposureProfile, xva::types::FundingConfig, xva::types::XvaResult,
    ConcentrationBreach, ImCollateralResult, ImResult, RepoMarginSpec, ScheduleAssetClass,
    SimmSensitivities, SimmVersion, VmResult,
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

#[cfg(feature = "json-schema")]
mod examples {
    //! One canonical payload per published margin contract.
    //!
    //! Calculator outputs are produced by running the calculator on the
    //! neighbouring input example, so the pair stays consistent.

    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::schema::{example, example_from_json};
    use finstack_quant_core::Result;
    use serde_json::{json, Value};

    fn usd(amount: i64) -> Money {
        Money::from((amount, Currency::USD))
    }

    pub(super) fn ccp_methodology() -> Result<Vec<Value>> {
        example(&crate::calculators::CcpMethodology::LchSwapClear)
    }

    pub(super) fn concentration_breach() -> Result<Vec<Value>> {
        example_from_json::<crate::ConcentrationBreach>(json!({
            "asset_class": "corporate_bonds",
            "fraction": 0.35,
            "limit": 0.25,
            "excess": 0.10
        }))
    }

    fn sa_ccr_trade_value() -> Result<crate::regulatory::SaCcrTrade> {
        serde_json::from_value(json!({
            "trade_id": "IRS-USD-5Y",
            "asset_class": "interest_rate",
            "supervisory_category": null,
            "option_maturity_date": null,
            "notional": 10_000_000.0,
            "start_date": "2025-01-15",
            "end_date": "2030-01-15",
            "underlier": "USD",
            "hedging_set": "USD",
            "direction": 1.0,
            "supervisory_delta": 1.0,
            "mtm": 125_000.0,
            "is_option": false,
            "option_type": null
        }))
        .map_err(|error| {
            finstack_quant_core::Error::Internal(format!("parse SA-CCR trade example: {error}"))
        })
    }

    fn sa_ccr_config_value() -> Result<crate::regulatory::SaCcrNettingSetConfig> {
        serde_json::from_value(json!({
            "as_of": "2025-01-15",
            "netting_set_id": {"kind": "bilateral", "counterparty_id": "ACME-BANK", "csa_id": "CSA-ACME-2024"},
            "is_margined": true,
            "collateral": 100_000.0,
            "threshold": 0.0,
            "mta": 50_000.0,
            "nica": 0.0,
            "mpor_days": 10
        }))
        .map_err(|error| {
            finstack_quant_core::Error::Internal(format!(
                "parse SA-CCR netting-set example: {error}"
            ))
        })
    }

    pub(super) fn sa_ccr_trade() -> Result<Vec<Value>> {
        example(&sa_ccr_trade_value()?)
    }

    pub(super) fn sa_ccr_netting_set_config() -> Result<Vec<Value>> {
        example(&sa_ccr_config_value()?)
    }

    /// The EAD of [`sa_ccr_trade`] under [`sa_ccr_netting_set_config`].
    pub(super) fn ead_result() -> Result<Vec<Value>> {
        example(&crate::regulatory::saccr_ead(
            &[sa_ccr_trade_value()?],
            &sa_ccr_config_value()?,
            None,
        )?)
    }

    pub(super) fn excess_collateral() -> Result<Vec<Value>> {
        example(&crate::metrics::ExcessCollateral::new(
            usd(1_250_000),
            usd(1_000_000),
        )?)
    }

    pub(super) fn exposure_profile() -> Result<Vec<Value>> {
        example_from_json::<crate::xva::types::ExposureProfile>(json!({
            "times": [0.0, 0.5, 1.0],
            "mtm_values": [0.0, 42_000.0, 31_000.0],
            "epe": [0.0, 85_000.0, 64_000.0],
            "ene": [0.0, -43_000.0, -33_000.0]
        }))
    }

    pub(super) fn frtb_risk_class() -> Result<Vec<Value>> {
        example(&crate::regulatory::FrtbRiskClass::Girr)
    }

    fn frtb_sensitivities_value() -> crate::regulatory::FrtbSensitivities {
        let mut sensitivities = crate::regulatory::FrtbSensitivities::new(Currency::USD);
        sensitivities.add_girr_delta(Currency::USD, "5Y", 12_500.0);
        sensitivities.add_girr_delta(Currency::USD, "10Y", -8_000.0);
        // Equity and FX deltas are P&L per one percent move.
        sensitivities.add_equity_delta("AAPL", 8, 2_500.0);
        sensitivities.add_fx_delta(Currency::EUR, Currency::USD, 4_000.0);
        sensitivities
    }

    pub(super) fn frtb_sensitivities() -> Result<Vec<Value>> {
        example(&frtb_sensitivities_value())
    }

    /// The SBA charge of [`frtb_sensitivities`] across all three scenarios.
    pub(super) fn frtb_sba_result() -> Result<Vec<Value>> {
        example(&crate::regulatory::frtb_sba_charge(
            &frtb_sensitivities_value(),
            None,
        )?)
    }

    pub(super) fn funding_config() -> Result<Vec<Value>> {
        example(&crate::xva::types::FundingConfig {
            funding_spread_bp: 85.0,
            funding_benefit_bp: Some(60.0),
            im_profile: None,
            margin_funding_spread_bp: None,
        })
    }

    pub(super) fn haircut01() -> Result<Vec<Value>> {
        example(&crate::metrics::Haircut01::calculate(usd(5_000_000), 0.02))
    }

    pub(super) fn im_collateral_result() -> Result<Vec<Value>> {
        example_from_json::<crate::ImCollateralResult>(json!({
            "gross_initial_margin": {"amount": "2500000", "currency": "USD"},
            "required_collateral": {"amount": "2500000", "currency": "USD"},
            "current_collateral": {"amount": "2000000", "currency": "USD"},
            "transfer": {"amount": "500000", "currency": "USD"},
            "segregated": true
        }))
    }

    pub(super) fn im_decay_profile() -> Result<Vec<Value>> {
        example(&crate::xva::mva::ImDecayProfile::LinearToMaturity {
            maturity_years: 5.0,
        })
    }

    pub(super) fn im_result() -> Result<Vec<Value>> {
        example_from_json::<crate::ImResult>(json!({
            "amount": {"amount": "2500000", "currency": "USD"},
            "methodology": "simm",
            "as_of": "2025-01-15",
            "mpor_days": 10,
            "breakdown": {
                "interest_rate": {"amount": "1900000", "currency": "USD"},
                "equity": {"amount": "600000", "currency": "USD"}
            },
            "approximation": false
        }))
    }

    pub(super) fn margin_constants() -> Result<Vec<Value>> {
        example(&crate::constants::MarginConstants::current())
    }

    pub(super) fn margin_funding_cost() -> Result<Vec<Value>> {
        example(&crate::metrics::MarginFundingCost::calculate(
            usd(10_000_000),
            0.055,
            0.0525,
        ))
    }

    pub(super) fn margin_utilization() -> Result<Vec<Value>> {
        example(&crate::metrics::MarginUtilization::new(
            usd(1_250_000),
            usd(1_000_000),
        )?)
    }

    pub(super) fn mva_result() -> Result<Vec<Value>> {
        example_from_json::<crate::xva::mva::MvaResult>(json!({
            "mva": 18_750.0,
            "average_im": 1_500_000.0,
            "im_profile": [[0.0, 2_500_000.0], [2.5, 1_250_000.0], [5.0, 0.0]]
        }))
    }

    pub(super) fn repo_margin_spec() -> Result<Vec<Value>> {
        example_from_json::<crate::RepoMarginSpec>(json!({
            "margin_type": "mark_to_market",
            "margin_ratio": 1.02,
            "margin_call_threshold": 0.01,
            "call_frequency": "daily",
            "settlement_lag": 1,
            "pays_margin_interest": true,
            "margin_interest_rate": 0.0525,
            "substitution_allowed": false
        }))
    }

    pub(super) fn schedule_asset_class() -> Result<Vec<Value>> {
        example(&crate::ScheduleAssetClass::InterestRate)
    }

    /// Equity, FX and commodity sensitivities.
    ///
    /// The rates and credit maps are keyed by tuples and have no JSON object
    /// form; [`simm_sensitivities_json`] is the wire shape that carries them.
    pub(super) fn simm_sensitivities() -> Result<Vec<Value>> {
        let mut sensitivities = crate::SimmSensitivities::new(Currency::USD);
        sensitivities.add_equity_delta("AAPL", 250_000.0);
        sensitivities.add_fx_delta(Currency::EUR, 400_000.0);
        example(&sensitivities)
    }

    pub(super) fn simm_sensitivities_json() -> Result<Vec<Value>> {
        let mut sensitivities = crate::SimmSensitivities::new(Currency::USD);
        sensitivities.add_ir_delta(Currency::USD, "5y", 12_500.0);
        sensitivities.add_ir_delta(Currency::USD, "10y", -8_000.0);
        sensitivities.add_equity_delta("AAPL", 250_000.0);
        sensitivities.add_fx_delta(Currency::EUR, 400_000.0);
        example(&crate::types::SimmSensitivitiesJson::from(&sensitivities))
    }

    pub(super) fn simm_version() -> Result<Vec<Value>> {
        example(&crate::SimmVersion::V2_6)
    }

    pub(super) fn vm_result() -> Result<Vec<Value>> {
        example_from_json::<crate::VmResult>(json!({
            "date": "2025-01-15",
            "gross_exposure": {"amount": "1750000", "currency": "USD"},
            "net_exposure": {"amount": "1250000", "currency": "USD"},
            "post_amount": {"amount": "0", "currency": "USD"},
            "collect_amount": {"amount": "1250000", "currency": "USD"},
            "settlement_date": "2025-01-16"
        }))
    }

    pub(super) fn xva_result() -> Result<Vec<Value>> {
        let meta = serde_json::to_value(finstack_quant_core::config::results_meta(
            &finstack_quant_core::config::FinstackConfig::default(),
        ))
        .map_err(|error| {
            finstack_quant_core::Error::Internal(format!("serialize XVA example meta: {error}"))
        })?;
        example_from_json::<crate::xva::types::XvaResult>(json!({
            "cva": 21_400.0,
            "dva": 6_300.0,
            "fva": 4_100.0,
            "total_xva": 19_200.0,
            "epe_profile": [[0.0, 0.0], [0.5, 85_000.0], [1.0, 64_000.0]],
            "ene_profile": [[0.0, 0.0], [0.5, -43_000.0], [1.0, -33_000.0]],
            "pfe_profile": [[0.0, 0.0], [0.5, 210_000.0], [1.0, 168_000.0]],
            "max_pfe": 210_000.0,
            "effective_epe_profile": [[0.0, 0.0], [0.5, 85_000.0], [1.0, 85_000.0]],
            "effective_epe": 63_750.0,
            "meta": meta
        }))
    }
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
    )
    .with_examples(examples::ccp_methodology),
    finstack_quant_core::schema_artifact!(
        ConcentrationBreach,
        "margin",
        "concentration_breach",
        Output,
        "A breach of a collateral concentration limit."
    )
    .with_examples(examples::concentration_breach),
    finstack_quant_core::schema_artifact!(
        EadResult,
        "margin",
        "ead_result",
        Output,
        "SA-CCR Exposure at Default result."
    )
    .with_examples(examples::ead_result),
    finstack_quant_core::schema_artifact!(
        ExcessCollateral,
        "margin",
        "excess_collateral",
        Output,
        "Excess collateral result."
    )
    .with_examples(examples::excess_collateral),
    finstack_quant_core::schema_artifact!(
        ExposureProfile,
        "margin",
        "exposure_profile",
        Input,
        "Exposure profile computed at each time grid point."
    )
    .with_examples(examples::exposure_profile),
    finstack_quant_core::schema_artifact!(
        FrtbRiskClass,
        "margin",
        "frtb_risk_class",
        Component,
        "FRTB risk classes per BCBS d457."
    )
    .with_examples(examples::frtb_risk_class),
    finstack_quant_core::schema_artifact!(
        FrtbSbaResult,
        "margin",
        "frtb_sba_result",
        Output,
        "Complete FRTB SBA capital charge result."
    )
    .with_examples(examples::frtb_sba_result),
    finstack_quant_core::schema_artifact!(
        FrtbSensitivities,
        "margin",
        "frtb_sensitivities",
        Input,
        "FRTB sensitivity inputs organized by risk class."
    )
    .with_examples(examples::frtb_sensitivities),
    finstack_quant_core::schema_artifact!(
        FundingConfig,
        "margin",
        "funding_config",
        Input,
        "Funding cost/benefit configuration for FVA and MVA calculation."
    )
    .with_examples(examples::funding_config),
    finstack_quant_core::schema_artifact!(
        Haircut01,
        "margin",
        "haircut01",
        Output,
        "Haircut sensitivity (Haircut01) result."
    )
    .with_examples(examples::haircut01),
    finstack_quant_core::schema_artifact!(
        ImCollateralResult,
        "margin",
        "im_collateral_result",
        Output,
        "One-way IM collateral account after applying the CSA's allocated threshold."
    )
    .with_examples(examples::im_collateral_result),
    finstack_quant_core::schema_artifact!(
        ImDecayProfile,
        "margin",
        "im_decay_profile",
        Input,
        "Deterministic decay applied to today's SIMM IM to approximate `E[IM(t)]`."
    )
    .with_examples(examples::im_decay_profile),
    finstack_quant_core::schema_artifact!(
        ImResult,
        "margin",
        "im_result",
        Output,
        "Initial margin calculation result."
    )
    .with_examples(examples::im_result),
    finstack_quant_core::schema_artifact!(
        MarginConstants,
        "margin",
        "margin_constants",
        Output,
        "Margin constants a host needs to interpret inputs and results."
    )
    .with_examples(examples::margin_constants),
    finstack_quant_core::schema_artifact!(
        MarginFundingCost,
        "margin",
        "margin_funding_cost",
        Output,
        "Margin funding cost result."
    )
    .with_examples(examples::margin_funding_cost),
    finstack_quant_core::schema_artifact!(
        MarginUtilization,
        "margin",
        "margin_utilization",
        Output,
        "Margin utilization result."
    )
    .with_examples(examples::margin_utilization),
    finstack_quant_core::schema_artifact!(
        MvaResult,
        "margin",
        "mva_result",
        Output,
        "Result of an MVA computation."
    )
    .with_examples(examples::mva_result),
    finstack_quant_core::schema_artifact!(
        RepoMarginSpec,
        "margin",
        "repo_margin_spec",
        Input,
        "GMRA 2011 compliant repo margin specification."
    )
    .with_examples(examples::repo_margin_spec),
    finstack_quant_core::schema_artifact!(
        SaCcrNettingSetConfig,
        "margin",
        "sa_ccr_netting_set_config",
        Input,
        "Netting set configuration for SA-CCR."
    )
    .with_examples(examples::sa_ccr_netting_set_config),
    finstack_quant_core::schema_artifact!(
        SaCcrTrade,
        "margin",
        "sa_ccr_trade",
        Input,
        "A single derivative trade for SA-CCR EAD computation."
    )
    .with_examples(examples::sa_ccr_trade),
    finstack_quant_core::schema_artifact!(
        ScheduleAssetClass,
        "margin",
        "schedule_asset_class",
        Component,
        "Asset class for schedule-based IM calculation."
    )
    .with_examples(examples::schedule_asset_class),
    finstack_quant_core::schema_artifact!(
        SimmSensitivities,
        "margin",
        "simm_sensitivities",
        Input,
        "SIMM sensitivity inputs organized by risk class."
    )
    .with_examples(examples::simm_sensitivities),
    finstack_quant_core::schema_artifact!(
        SimmSensitivitiesJson,
        "margin",
        "simm_sensitivities_json",
        Input,
        "JSON-friendly representation of `SimmSensitivities`."
    )
    .with_examples(examples::simm_sensitivities_json),
    finstack_quant_core::schema_artifact!(
        SimmVersion,
        "margin",
        "simm_version",
        Component,
        "SIMM version identifier."
    )
    .with_examples(examples::simm_version),
    finstack_quant_core::schema_artifact!(
        VmResult,
        "margin",
        "vm_result",
        Output,
        "Variation margin calculation result."
    )
    .with_examples(examples::vm_result),
    finstack_quant_core::schema_artifact!(
        XvaResult,
        "margin",
        "xva_result",
        Output,
        "Result of XVA calculations."
    )
    .with_examples(examples::xva_result),
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
