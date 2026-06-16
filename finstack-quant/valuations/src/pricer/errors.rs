//! Pricing error types and context utilities.
//!
//! Defines [`PricingError`] and [`PricingErrorContext`].

use super::{InstrumentType, ModelKey, PricerKey};
use crate::instruments::common_impl::traits::Instrument as Priceable;

/// Context for pricing operations, providing actionable debugging information.
///
/// This struct captures the instrument, model, and market data context
/// when a pricing error occurs, enabling easier troubleshooting.
///
/// Serde is derived because this type flows through the crate-level
/// [`crate::Error`] enum, which is part of the wire-stable error envelope.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PricingErrorContext {
    /// The instrument ID that was being priced (if known).
    pub instrument_id: Option<String>,
    /// The instrument type being priced.
    pub instrument_type: Option<InstrumentType>,
    /// The pricing model being used.
    pub model: Option<ModelKey>,
    /// Market data curve/surface IDs involved in the operation.
    pub curve_ids: Vec<String>,
}

impl PricingErrorContext {
    /// Create a new empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create context from an instrument, capturing ID and type.
    ///
    /// This is a convenience method to reduce boilerplate when building
    /// error context in pricer implementations.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let ctx = PricingErrorContext::from_instrument(bond)
    ///     .model(ModelKey::Discounting)
    ///     .curve_id("USD-OIS");
    /// ```
    pub fn from_instrument(instrument: &dyn Priceable) -> Self {
        Self {
            instrument_id: Some(instrument.id().to_string()),
            instrument_type: Some(instrument.key()),
            ..Default::default()
        }
    }

    /// Set the instrument ID.
    pub fn instrument_id(mut self, id: impl Into<String>) -> Self {
        self.instrument_id = Some(id.into());
        self
    }

    /// Set the instrument type.
    pub fn instrument_type(mut self, typ: InstrumentType) -> Self {
        self.instrument_type = Some(typ);
        self
    }

    /// Set the pricing model.
    pub fn model(mut self, model: ModelKey) -> Self {
        self.model = Some(model);
        self
    }

    /// Add a curve/surface ID to the context.
    pub fn curve_id(mut self, curve_id: impl Into<String>) -> Self {
        self.curve_ids.push(curve_id.into());
        self
    }

    /// Add multiple curve/surface IDs to the context.
    pub fn curve_ids(mut self, curve_ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.curve_ids
            .extend(curve_ids.into_iter().map(|s| s.into()));
        self
    }
}

impl std::fmt::Display for PricingErrorContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = Vec::new();
        if let Some(ref id) = self.instrument_id {
            parts.push(format!("instrument={}", id));
        }
        if let Some(typ) = self.instrument_type {
            parts.push(format!("type={:?}", typ));
        }
        if let Some(model) = self.model {
            parts.push(format!("model={:?}", model));
        }
        if !self.curve_ids.is_empty() {
            parts.push(format!("curves=[{}]", self.curve_ids.join(", ")));
        }
        if parts.is_empty() {
            write!(f, "<no context>")
        } else {
            write!(f, "{}", parts.join(", "))
        }
    }
}

/// Pricing-specific errors returned by pricer implementations.
///
/// Each variant captures the error condition along with optional context
/// (instrument ID, type, model, and curve IDs) for actionable debugging.
///
/// Serde is derived because this type flows through the crate-level
/// [`crate::Error`] enum, which is part of the wire-stable error envelope.
#[derive(Debug, Clone, PartialEq, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum PricingError {
    /// No pricer registered for the requested (instrument, model) combination.
    ///
    /// `available_models` lists every model that IS registered for the
    /// requested instrument type, so analysts can recover from the typo or
    /// the missing feature-flag without re-reading registration source.
    /// Empty when the instrument type has no pricers at all (e.g. unknown
    /// instrument or a registry built without standard pricers).
    #[error(
        "No pricer found for instrument={} model={}{}",
        .key.instrument,
        .key.model,
        format_available_models(.available_models),
    )]
    UnknownPricer {
        /// The (instrument, model) pair that had no registered pricer.
        key: PricerKey,
        /// Models registered for `key.instrument` in the registry that produced
        /// this error. Empty if no pricers exist for the instrument type.
        available_models: Vec<ModelKey>,
    },

    /// Instrument type mismatch during downcasting.
    #[error("Type mismatch: expected {expected}, got {got}")]
    TypeMismatch {
        /// Expected instrument type
        expected: InstrumentType,
        /// Actual instrument type
        got: InstrumentType,
    },

    /// Pricing model computation failed.
    ///
    /// The context provides actionable information about which instrument
    /// and model were involved when the failure occurred.
    #[error("Model failure: {message}{}", format_context(.context))]
    ModelFailure {
        /// Error message describing the failure.
        message: String,
        /// Context: instrument, model, and curves involved.
        context: PricingErrorContext,
    },

    /// Invalid input parameters provided.
    ///
    /// The context identifies which instrument had invalid inputs.
    #[error("Invalid input: {message}{}", format_context(.context))]
    InvalidInput {
        /// Error message describing the invalid input.
        message: String,
        /// Context: instrument and relevant details.
        context: PricingErrorContext,
    },

    /// Missing market data required for pricing.
    ///
    /// Identifies exactly which market data ID is missing and for which instrument.
    #[error("Missing market data: {missing_id} required for pricing{}", format_context(.context))]
    MissingMarketData {
        /// The ID of the missing market data (curve, surface, or scalar).
        missing_id: String,
        /// Context: instrument requiring this data.
        context: PricingErrorContext,
    },
}

/// Helper to format the list of available models on an `UnknownPricer` error.
///
/// Returns an empty string when no models are registered, otherwise a
/// `". Available models for this instrument: ..."` suffix that piggy-backs
/// on the base error message.
fn format_available_models(models: &[ModelKey]) -> String {
    if models.is_empty() {
        String::new()
    } else {
        let names: Vec<String> = models.iter().map(ModelKey::to_string).collect();
        format!(
            ". Available models for this instrument: {}",
            names.join(", ")
        )
    }
}

/// Helper to format context for error display.
fn format_context(ctx: &PricingErrorContext) -> String {
    let display = ctx.to_string();
    if display == "<no context>" {
        String::new()
    } else {
        format!(" [{}]", display)
    }
}

/// Provide a more actionable error for missing Monte Carlo model registrations.
///
/// `available_models` is appended to the message so analysts see which models
/// they could use as fallbacks. Pass an empty slice when no registry context
/// is available.
pub(crate) fn actionable_unknown_pricer_message(
    key: PricerKey,
    available_models: &[ModelKey],
) -> Option<String> {
    if key.model.is_monte_carlo_model() {
        let extra_hint = match (key.instrument, key.model) {
            (InstrumentType::BarrierOption, ModelKey::MonteCarloGBM)
            | (InstrumentType::LookbackOption, ModelKey::MonteCarloGBM)
            | (InstrumentType::FxBarrierOption, ModelKey::MonteCarloGBM) => {
                " Switch the instrument back to its continuous-monitoring configuration, or register a compatible Monte Carlo pricer."
            }
            (InstrumentType::BermudanSwaption, ModelKey::MonteCarloHullWhite1F) => {
                " Select a non-LSMC pricing model, or register a compatible Monte Carlo pricer."
            }
            _ => " Register a compatible Monte Carlo pricer or select one of the available models.",
        };
        return Some(format!(
            "No pricer found for instrument={} model={}.{}{}",
            key.instrument,
            key.model,
            extra_hint,
            format_available_models(available_models),
        ));
    }

    None
}

/// Lossy conversion from [`PricingError`] into [`finstack_quant_core::Error`].
///
/// This mapping is intentionally lossy — pricing-specific context (instrument ID,
/// model, curve IDs) is flattened into core error messages.  The mapping must be
/// kept in sync with `PricingError` variants; add a new arm whenever a variant is
/// added.
///
/// | `PricingError`        | `finstack_quant_core::Error`          | What is lost                          |
/// |-----------------------|---------------------------------|---------------------------------------|
/// | `UnknownPricer`       | `Input(NotFound)`               | Typed `PricerKey` → string id         |
/// | `TypeMismatch`        | `Input(Invalid)`                | Expected/got instrument types         |
/// | `InvalidInput`        | `Validation`                    | Structured `PricingErrorContext`       |
/// | `MissingMarketData`   | `Input(NotFound)`               | `PricingErrorContext`                  |
/// | `ModelFailure`        | `Calibration`                   | `PricingErrorContext`; category fixed  |
impl From<PricingError> for finstack_quant_core::Error {
    fn from(err: PricingError) -> Self {
        match err {
            PricingError::UnknownPricer { key, .. } => {
                let pricer_id = format!("pricer:{}:{:?}", key.instrument, key.model);
                finstack_quant_core::InputError::NotFound { id: pricer_id }.into()
            }
            PricingError::TypeMismatch { .. } => finstack_quant_core::InputError::Invalid.into(),
            PricingError::InvalidInput { message, context } => {
                finstack_quant_core::Error::Validation(format!(
                    "{message}{}",
                    format_context(&context)
                ))
            }
            PricingError::MissingMarketData { missing_id, .. } => {
                finstack_quant_core::InputError::NotFound { id: missing_id }.into()
            }
            PricingError::ModelFailure { message, context } => {
                finstack_quant_core::Error::Calibration {
                    message: format!("{message}{}", format_context(&context)),
                    category: "pricing_model".to_string(),
                }
            }
        }
    }
}

impl PricingError {
    /// Convert a [`finstack_quant_core::Error`] into a [`PricingError`] with explicit
    /// context.
    ///
    /// This replaces the former blanket `From<finstack_quant_core::Error>` impl, which
    /// was lossy — every conversion silently attached an empty
    /// [`PricingErrorContext`].  By requiring context as a parameter, callers are
    /// forced to provide actionable debugging information.
    ///
    /// # Mapping
    ///
    /// | `finstack_quant_core::Error`           | `PricingError`      |
    /// |----------------------------------|---------------------|
    /// | `Input(NotFound { id })`         | `MissingMarketData` |
    /// | `Input(MissingCurve { .. })`     | `MissingMarketData` |
    /// | `Input(WrongCurveType { .. })`   | `InvalidInput`      |
    /// | `Input(other)`                   | `InvalidInput`      |
    /// | `Validation(msg)`                | `InvalidInput`      |
    /// | `Calibration { message, .. }`    | `ModelFailure`      |
    /// | all other variants               | `ModelFailure`      |
    ///
    /// # Example
    ///
    /// ```ignore
    /// let core_err: finstack_quant_core::Error = /* ... */;
    /// let ctx = PricingErrorContext::new()
    ///     .instrument_id("BOND-001")
    ///     .instrument_type(InstrumentType::Bond)
    ///     .model(ModelKey::Discounting);
    /// let pricing_err = PricingError::from_core(core_err, ctx);
    /// ```
    pub fn from_core(err: finstack_quant_core::Error, context: PricingErrorContext) -> Self {
        match err {
            finstack_quant_core::Error::Input(input) => match input {
                finstack_quant_core::InputError::NotFound { id } => {
                    PricingError::MissingMarketData {
                        missing_id: id,
                        context,
                    }
                }
                finstack_quant_core::InputError::MissingCurve { requested, .. } => {
                    PricingError::MissingMarketData {
                        missing_id: requested,
                        context,
                    }
                }
                finstack_quant_core::InputError::WrongCurveType {
                    id,
                    expected,
                    actual,
                } => PricingError::InvalidInput {
                    message: format!(
                        "Curve type mismatch for '{id}': expected '{expected}', got '{actual}'"
                    ),
                    context,
                },
                other => PricingError::InvalidInput {
                    message: other.to_string(),
                    context,
                },
            },
            finstack_quant_core::Error::Validation(msg) => PricingError::InvalidInput {
                message: msg,
                context,
            },
            finstack_quant_core::Error::Calibration { message, .. } => {
                PricingError::ModelFailure { message, context }
            }
            other => PricingError::ModelFailure {
                message: other.to_string(),
                context,
            },
        }
    }
    /// Create a type mismatch error.
    pub fn type_mismatch(expected: InstrumentType, got: InstrumentType) -> Self {
        Self::TypeMismatch { expected, got }
    }

    /// Create a model failure error with full context.
    ///
    /// # Example
    ///
    /// ```ignore
    /// PricingError::model_failure_with_context(
    ///     "Discount factor calculation failed",
    ///     PricingErrorContext::new()
    ///         .instrument_id("BOND-001")
    ///         .instrument_type(InstrumentType::Bond)
    ///         .model(ModelKey::Discounting)
    ///         .curve_id("USD-OIS"),
    /// )
    /// ```
    pub fn model_failure_with_context(
        msg: impl Into<String>,
        context: PricingErrorContext,
    ) -> Self {
        Self::ModelFailure {
            message: msg.into(),
            context,
        }
    }

    /// Create an invalid input error with full context.
    pub fn invalid_input_with_context(
        msg: impl Into<String>,
        context: PricingErrorContext,
    ) -> Self {
        Self::InvalidInput {
            message: msg.into(),
            context,
        }
    }

    /// Create a missing market data error with the specific missing ID and context.
    ///
    /// # Example
    ///
    /// ```ignore
    /// PricingError::missing_market_data_with_context(
    ///     "USD-OIS",
    ///     PricingErrorContext::new()
    ///         .instrument_id("BOND-001")
    ///         .instrument_type(InstrumentType::Bond),
    /// )
    /// ```
    pub fn missing_market_data_with_context(
        missing_id: impl Into<String>,
        context: PricingErrorContext,
    ) -> Self {
        Self::MissingMarketData {
            missing_id: missing_id.into(),
            context,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pricing_error_maps_to_structured_core_errors() {
        // MissingMarketData -> InputError::NotFound
        let missing: finstack_quant_core::Error = PricingError::MissingMarketData {
            missing_id: "USD-SOFR".to_string(),
            context: PricingErrorContext::default(),
        }
        .into();
        match missing {
            finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound { id }) => {
                assert_eq!(id, "USD-SOFR")
            }
            other => panic!("unexpected mapping for missing market data: {other:?}"),
        }

        // UnknownPricer -> InputError::NotFound
        let unknown_pricer: finstack_quant_core::Error = PricingError::UnknownPricer {
            key: PricerKey::new(InstrumentType::Bond, ModelKey::Tree),
            available_models: Vec::new(),
        }
        .into();
        match unknown_pricer {
            finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound { id }) => {
                assert_eq!(id, "pricer:bond:Tree")
            }
            other => panic!("unexpected mapping for unknown pricer: {other:?}"),
        }

        // TypeMismatch -> InputError::Invalid
        let type_mismatch: finstack_quant_core::Error = PricingError::TypeMismatch {
            expected: InstrumentType::Bond,
            got: InstrumentType::IRS,
        }
        .into();
        match type_mismatch {
            finstack_quant_core::Error::Input(finstack_quant_core::InputError::Invalid) => {}
            other => panic!("unexpected mapping for type mismatch: {other:?}"),
        }

        // InvalidInput -> Error::Validation (not Calibration)
        let invalid_input: finstack_quant_core::Error = PricingError::InvalidInput {
            message: "bad parameter".to_string(),
            context: PricingErrorContext::new().instrument_id("TEST-001"),
        }
        .into();
        match invalid_input {
            finstack_quant_core::Error::Validation(msg) => {
                assert!(
                    msg.contains("bad parameter"),
                    "Validation message should contain original message"
                );
                assert!(
                    msg.contains("TEST-001"),
                    "Validation message should contain context"
                );
            }
            other => panic!("unexpected mapping for invalid input: {other:?}"),
        }

        // ModelFailure -> Calibration (for numerical/solver failures)
        let model_failure: finstack_quant_core::Error = PricingError::ModelFailure {
            message: "solver did not converge".to_string(),
            context: PricingErrorContext::default(),
        }
        .into();
        match model_failure {
            finstack_quant_core::Error::Calibration { category, message } => {
                assert_eq!(category, "pricing_model");
                assert!(message.contains("solver did not converge"));
            }
            other => panic!("unexpected mapping for model failure: {other:?}"),
        }
    }

    #[test]
    fn from_core_maps_error_categories_with_context() {
        let ctx = PricingErrorContext::new()
            .instrument_id("TEST-001")
            .instrument_type(InstrumentType::Bond);

        // Input::NotFound -> MissingMarketData (preserves context)
        let core_missing: finstack_quant_core::Error = finstack_quant_core::InputError::NotFound {
            id: "USD-OIS".to_string(),
        }
        .into();
        let pricing = PricingError::from_core(core_missing, ctx.clone());
        match &pricing {
            PricingError::MissingMarketData {
                missing_id,
                context,
            } => {
                assert_eq!(missing_id, "USD-OIS");
                assert_eq!(
                    context.instrument_id.as_deref(),
                    Some("TEST-001"),
                    "context should be preserved"
                );
            }
            other => panic!("unexpected mapping for missing input: {other:?}"),
        }

        // Validation -> InvalidInput (not ModelFailure)
        let core_invalid = finstack_quant_core::Error::Validation("bad parameter".to_string());
        let pricing = PricingError::from_core(core_invalid, ctx.clone());
        match pricing {
            PricingError::InvalidInput { message, .. } => {
                assert!(message.contains("bad parameter"));
            }
            other => panic!("unexpected mapping for validation: {other:?}"),
        }

        // Calibration -> ModelFailure
        let core_calibration = finstack_quant_core::Error::Calibration {
            message: "solver did not converge".to_string(),
            category: "solver".to_string(),
        };
        let pricing = PricingError::from_core(core_calibration, ctx);
        match pricing {
            PricingError::ModelFailure { message, .. } => {
                assert!(message.contains("solver did not converge"));
            }
            other => panic!("unexpected mapping for calibration: {other:?}"),
        }
    }

    #[test]
    fn pricing_error_context_display_covers_empty_and_populated_forms() {
        let empty = PricingErrorContext::new();
        assert_eq!(empty.to_string(), "<no context>");

        let populated = PricingErrorContext::new()
            .instrument_id("BOND-007")
            .instrument_type(InstrumentType::Bond)
            .model(ModelKey::Discounting)
            .curve_id("USD-OIS")
            .curve_ids(["USD-SOFR", "USD-CREDIT"]);

        let rendered = populated.to_string();
        assert!(rendered.contains("instrument=BOND-007"));
        assert!(rendered.contains("type=Bond"));
        assert!(rendered.contains("model=Discounting"));
        assert!(rendered.contains("curves=[USD-OIS, USD-SOFR, USD-CREDIT]"));
    }

    #[test]
    fn from_core_maps_missing_curve_wrong_curve_type_and_fallback_inputs() {
        let ctx = PricingErrorContext::new()
            .instrument_id("TEST-002")
            .instrument_type(InstrumentType::IRS)
            .model(ModelKey::Discounting);

        let missing_curve = PricingError::from_core(
            finstack_quant_core::InputError::MissingCurve {
                requested: "USD-OIS".into(),
                suggestions: vec!["USD-SOFR".into()],
            }
            .into(),
            ctx.clone(),
        );
        match missing_curve {
            PricingError::MissingMarketData {
                missing_id,
                context,
            } => {
                assert_eq!(missing_id, "USD-OIS");
                assert_eq!(context.instrument_id.as_deref(), Some("TEST-002"));
            }
            other => panic!("expected MissingMarketData, got {other:?}"),
        }

        let wrong_curve_type = PricingError::from_core(
            finstack_quant_core::InputError::WrongCurveType {
                id: "USD-OIS".into(),
                expected: "DiscountCurve".into(),
                actual: "HazardCurve".into(),
            }
            .into(),
            ctx.clone(),
        );
        match wrong_curve_type {
            PricingError::InvalidInput { message, context } => {
                assert!(message.contains("Curve type mismatch"));
                assert!(message.contains("DiscountCurve"));
                assert!(message.contains("HazardCurve"));
                assert_eq!(context.instrument_type, Some(InstrumentType::IRS));
            }
            other => panic!("expected InvalidInput, got {other:?}"),
        }

        let fallback_input =
            PricingError::from_core(finstack_quant_core::InputError::Invalid.into(), ctx);
        match fallback_input {
            PricingError::InvalidInput { message, .. } => {
                assert!(message.contains("Invalid input data"));
            }
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn error_builder_helpers_preserve_payloads() {
        let context = PricingErrorContext::new()
            .instrument_id("BOND-123")
            .instrument_type(InstrumentType::Bond)
            .model(ModelKey::Discounting)
            .curve_id("USD-OIS");
        let invalid = PricingError::invalid_input_with_context("bad fixing", context);

        match invalid {
            PricingError::InvalidInput { message, context } => {
                assert_eq!(message, "bad fixing");
                assert_eq!(context.instrument_id.as_deref(), Some("BOND-123"));
                assert_eq!(context.instrument_type, Some(InstrumentType::Bond));
                assert_eq!(context.model, Some(ModelKey::Discounting));
                assert_eq!(context.curve_ids, vec!["USD-OIS".to_string()]);
            }
            other => panic!("expected InvalidInput, got {other:?}"),
        }

        let missing = PricingError::missing_market_data_with_context(
            "EUR-OIS",
            PricingErrorContext::new().instrument_id("SWAP-1"),
        );
        match missing {
            PricingError::MissingMarketData {
                missing_id,
                context,
            } => {
                assert_eq!(missing_id, "EUR-OIS");
                assert_eq!(context.instrument_id.as_deref(), Some("SWAP-1"));
            }
            other => panic!("expected MissingMarketData, got {other:?}"),
        }

        let type_mismatch = PricingError::type_mismatch(InstrumentType::Bond, InstrumentType::IRS);
        assert!(matches!(type_mismatch, PricingError::TypeMismatch { .. }));
    }
}
