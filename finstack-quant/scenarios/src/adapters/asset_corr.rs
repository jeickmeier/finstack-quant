//! Asset correlation shock adapter for structured credit instruments.
//!
//! These adapters return engine-internal [`ScenarioEffect`]s that the engine
//! later applies in batch via the structured-credit instrument downcast in
//! [`crate::engine::ScenarioEngine::apply`].
//!
//! # Clamping
//!
//! Correlation values are clamped to valid ranges after shocks:
//! - Asset correlation: [0, 0.99]
//! - Prepay-default correlation: [-0.99, 0.99]

use crate::adapters::traits::ScenarioEffect;

/// Generate an effect for an asset-correlation parallel shock.
pub(crate) fn asset_corr_effects(delta_pts: f64) -> Vec<ScenarioEffect> {
    vec![ScenarioEffect::AssetCorrelationShock { delta_pts }]
}

/// Generate an effect for a prepay-default-correlation parallel shock.
pub(crate) fn prepay_default_corr_effects(delta_pts: f64) -> Vec<ScenarioEffect> {
    vec![ScenarioEffect::PrepayDefaultCorrelationShock { delta_pts }]
}

#[cfg(test)]
mod tests {
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
        CorrelationStructure, StructuredCredit,
    };

    /// Test-only helper: apply a parallel shock to asset correlation.
    fn apply_asset_correlation_shock(
        instruments: &mut [StructuredCredit],
        shock_points: f64,
    ) -> (usize, Vec<String>) {
        let mut modified = 0;
        let mut warnings = Vec::new();

        for inst in instruments.iter_mut() {
            if let Some(ref corr_structure) = inst.credit_model.correlation_structure {
                let (new_corr_structure, clamp_info) =
                    corr_structure.bump_asset_with_clamp_info(shock_points);
                if let Some(info) = clamp_info {
                    warnings.push(format!("Asset correlation bump for '{}': {info}", inst.id));
                }
                inst.credit_model.correlation_structure = Some(new_corr_structure);
                modified += 1;
            }
        }

        (modified, warnings)
    }

    /// Test-only helper: apply a parallel shock to prepay-default correlation.
    fn apply_prepay_default_correlation_shock(
        instruments: &mut [StructuredCredit],
        shock_points: f64,
    ) -> (usize, Vec<String>) {
        let mut modified = 0;
        let mut warnings = Vec::new();

        for inst in instruments.iter_mut() {
            if let Some(ref corr_structure) = inst.credit_model.correlation_structure {
                let (new_corr_structure, clamp_info) =
                    corr_structure.bump_prepay_default_with_clamp_info(shock_points);
                if let Some(info) = clamp_info {
                    warnings.push(format!(
                        "Prepay-default correlation bump for '{}': {info}",
                        inst.id
                    ));
                }
                inst.credit_model.correlation_structure = Some(new_corr_structure);
                modified += 1;
            }
        }

        (modified, warnings)
    }

    fn sample_instrument() -> StructuredCredit {
        let mut inst = StructuredCredit::example();
        inst.credit_model.correlation_structure = Some(CorrelationStructure::flat(0.20, -0.30));
        inst
    }

    #[test]
    fn test_asset_correlation_shock() {
        let mut instruments = vec![sample_instrument()];
        let (modified, _) = apply_asset_correlation_shock(&mut instruments, 0.05);

        assert_eq!(modified, 1);

        let new_corr = instruments[0]
            .credit_model
            .correlation_structure
            .as_ref()
            .expect("should have correlation")
            .asset_correlation();
        assert!((new_corr - 0.25).abs() < 0.01);
    }

    #[test]
    fn test_asset_correlation_clamping() {
        let mut instruments = vec![sample_instrument()];
        let (_, warnings) = apply_asset_correlation_shock(&mut instruments, 0.90);

        assert!(!warnings.is_empty(), "Should have clamping warning");
        let new_corr = instruments[0]
            .credit_model
            .correlation_structure
            .as_ref()
            .expect("should have correlation")
            .asset_correlation();
        assert!(new_corr <= 0.99);
    }

    #[test]
    fn test_prepay_default_correlation_shock() {
        let mut instruments = vec![sample_instrument()];
        let (modified, _) = apply_prepay_default_correlation_shock(&mut instruments, 0.10);

        assert_eq!(modified, 1);
        let new_corr = instruments[0]
            .credit_model
            .correlation_structure
            .as_ref()
            .expect("should have correlation")
            .prepay_default_correlation();
        assert!((new_corr - (-0.20)).abs() < 0.01);
    }
}
