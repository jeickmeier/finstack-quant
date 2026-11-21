//! Model parameter snapshots and extraction/application for instrument-level attribution.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::{Error, Result};
use finstack_quant_valuations::instruments::fixed_income::convertible::ConvertibleBond;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    StructuredCredit, StructuredCreditTranche,
};
use finstack_quant_valuations::instruments::{ConversionSpec, Instrument};
use std::sync::Arc;

/// Snapshot of extractable model parameters from an instrument.
///
/// Different instrument types have different model parameters that affect
/// pricing. This enum captures the relevant parameters for each type.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ModelParamsSnapshot {
    /// Structured credit parameters (prepayment, default, recovery).
    StructuredCredit {
        /// Prepayment model specification.
        prepayment_spec: PrepaymentModelSpec,
        /// Default model specification.
        default_spec: DefaultModelSpec,
        /// Recovery model specification.
        recovery_spec: RecoveryModelSpec,
    },

    /// Convertible bond parameters (conversion ratio, policies).
    Convertible {
        /// Conversion specification for convertible bonds.
        conversion_spec: ConversionSpec,
    },

    /// No extractable model parameters.
    None,
}

/// Extract the attribution model-parameter snapshot owned by `instrument`.
pub(crate) fn extract_model_params(instrument: &dyn Instrument) -> ModelParamsSnapshot {
    let any = instrument.as_any();
    if let Some(deal) = any.downcast_ref::<StructuredCredit>() {
        return ModelParamsSnapshot::StructuredCredit {
            prepayment_spec: deal.credit_model.prepayment_spec.clone(),
            default_spec: deal.credit_model.default_spec.clone(),
            recovery_spec: deal.credit_model.recovery_spec.clone(),
        };
    }
    if let Some(tranche) = any.downcast_ref::<StructuredCreditTranche>() {
        let deal = tranche.deal();
        return ModelParamsSnapshot::StructuredCredit {
            prepayment_spec: deal.credit_model.prepayment_spec.clone(),
            default_spec: deal.credit_model.default_spec.clone(),
            recovery_spec: deal.credit_model.recovery_spec.clone(),
        };
    }
    if let Some(convertible) = any.downcast_ref::<ConvertibleBond>() {
        return ModelParamsSnapshot::Convertible {
            conversion_spec: convertible.conversion.clone(),
        };
    }
    #[cfg(test)]
    if let Some(snapshot) = crate::taylor::tests::extract_mock_model_params(instrument) {
        return snapshot;
    }
    ModelParamsSnapshot::None
}

/// Rebuild `instrument` with `params` applied when supported, or clone `instrument` for [`ModelParamsSnapshot::None`].
pub(crate) fn instrument_with_model_params(
    instrument: &Arc<dyn Instrument>,
    params: &ModelParamsSnapshot,
) -> Result<Arc<dyn Instrument>> {
    let any = instrument.as_any();
    match params {
        ModelParamsSnapshot::None => Ok(Arc::clone(instrument)),
        ModelParamsSnapshot::StructuredCredit {
            prepayment_spec,
            default_spec,
            recovery_spec,
        } => {
            if let Some(deal) = any.downcast_ref::<StructuredCredit>() {
                let modified = deal.with_model_assumptions(
                    prepayment_spec.clone(),
                    default_spec.clone(),
                    recovery_spec.clone(),
                )?;
                return Ok(Arc::new(modified));
            }
            if let Some(tranche) = any.downcast_ref::<StructuredCreditTranche>() {
                let deal = tranche.deal().with_model_assumptions(
                    prepayment_spec.clone(),
                    default_spec.clone(),
                    recovery_spec.clone(),
                )?;
                return Ok(Arc::new(tranche.with_deal(deal)?));
            }
            #[cfg(test)]
            if let Some(result) = crate::taylor::tests::apply_mock_model_params(instrument, params)
            {
                return result;
            }
            Err(Error::Validation(if any.is::<ConvertibleBond>() {
                "Instrument type mismatch: expected ConvertibleBond model parameters".to_string()
            } else {
                format!(
                    "Instrument '{}' ({:?}) does not support model parameter replacement",
                    instrument.id(),
                    instrument.key()
                )
            }))
        }
        ModelParamsSnapshot::Convertible { conversion_spec } => {
            if let Some(convertible) = any.downcast_ref::<ConvertibleBond>() {
                let mut modified = convertible.clone();
                modified.conversion = conversion_spec.clone();
                return Ok(Arc::new(modified));
            }
            #[cfg(test)]
            if let Some(result) = crate::taylor::tests::apply_mock_model_params(instrument, params)
            {
                return result;
            }
            Err(Error::Validation(
                if any.is::<StructuredCredit>() || any.is::<StructuredCreditTranche>() {
                    "Instrument type mismatch: expected StructuredCredit model parameters"
                        .to_string()
                } else {
                    format!(
                        "Instrument '{}' ({:?}) does not support model parameter replacement",
                        instrument.id(),
                        instrument.key()
                    )
                },
            ))
        }
    }
}
