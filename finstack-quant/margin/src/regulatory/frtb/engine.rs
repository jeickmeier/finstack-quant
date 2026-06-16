//! FRTB Sensitivity-Based Approach engine.
//!
//! Computes the standardized market risk capital charge per BCBS d457.
//! The engine is configured at build time; all parameter validation
//! happens in the builder so `calculate()` is infallible given valid inputs.

use super::aggregation::aggregate_sba;
use super::curvature::curvature_charge;
use super::delta::delta_charge;
use super::drc::drc_charge;
use super::rrao::rrao_charge;
use super::types::{CorrelationScenario, FrtbRiskClass, FrtbSbaResult, FrtbSensitivities};
use super::vega::vega_charge;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::HashMap;
use finstack_quant_core::Result;

/// FRTB Sensitivity-Based Approach engine.
///
/// Computes the standardized market risk capital charge per BCBS d457.
/// The engine carries a revision-tagged [`super::params::FrtbParams`]
/// bundle so audit logs can match a result to its regulatory vintage;
/// charge-calculation helpers currently read the `pub const` tables in
/// [`super::params`] directly.
#[derive(Debug)]
pub struct FrtbSbaEngine {
    /// Which correlation scenarios to evaluate (default: all three).
    scenarios: Vec<CorrelationScenario>,
    /// Which risk classes to include (default: all).
    risk_classes: Vec<FrtbRiskClass>,
    /// Base currency for reporting.
    #[allow(dead_code)]
    reporting_currency: Currency,
    /// Active FRTB parameter set (revision-tagged).
    params: super::params::FrtbParams,
}

impl FrtbSbaEngine {
    /// Revision tag of the active parameter set, for audit stamping.
    #[must_use]
    pub fn parameter_revision(&self) -> &super::params::FrtbRevision {
        &self.params.revision
    }

    /// The active parameter bundle.
    #[must_use]
    pub fn parameters(&self) -> &super::params::FrtbParams {
        &self.params
    }
}

impl FrtbSbaEngine {
    /// Create a builder for configuring the engine.
    #[must_use]
    pub fn builder() -> FrtbSbaEngineBuilder {
        FrtbSbaEngineBuilder::default()
    }

    /// Compute the full FRTB SBA capital charge.
    ///
    /// Evaluates delta, vega, and curvature charges under each configured
    /// correlation scenario, takes the maximum, then adds DRC and RRAO.
    pub fn calculate(&self, sensitivities: &FrtbSensitivities) -> Result<FrtbSbaResult> {
        let mut scenario_charges: HashMap<CorrelationScenario, f64> = HashMap::default();
        let mut best_scenario = CorrelationScenario::Medium;
        let mut max_sba_charge = f64::NEG_INFINITY;

        // Charges from the binding (max) scenario are stored for the result.
        let mut best_delta: HashMap<FrtbRiskClass, f64> = HashMap::default();
        let mut best_vega: HashMap<FrtbRiskClass, f64> = HashMap::default();
        let mut best_curvature: HashMap<FrtbRiskClass, f64> = HashMap::default();

        for &scenario in &self.scenarios {
            let mut delta_charges = HashMap::default();
            let mut vega_charges = HashMap::default();
            let mut curvature_charges = HashMap::default();

            for &rc in &self.risk_classes {
                let d = delta_charge(rc, sensitivities, scenario);
                let v = vega_charge(rc, sensitivities, scenario);
                let c = curvature_charge(rc, sensitivities, scenario);

                if d > 0.0 {
                    delta_charges.insert(rc, d);
                }
                if v > 0.0 {
                    vega_charges.insert(rc, v);
                }
                if c > 0.0 {
                    curvature_charges.insert(rc, c);
                }
            }

            let sba_agg = aggregate_sba(&delta_charges, &vega_charges, &curvature_charges);
            scenario_charges.insert(scenario, sba_agg);

            if sba_agg > max_sba_charge {
                max_sba_charge = sba_agg;
                best_scenario = scenario;
                best_delta = delta_charges;
                best_vega = vega_charges;
                best_curvature = curvature_charges;
            }
        }

        // DRC and RRAO are not subject to correlation scenarios.
        let drc = drc_charge(&sensitivities.drc_positions);
        let rrao = rrao_charge(&sensitivities.rrao_exotic_notionals);

        // Total = max(SBA across scenarios) + DRC + RRAO.
        let total = f64::max(max_sba_charge, 0.0) + drc + rrao;

        Ok(FrtbSbaResult {
            total,
            delta_by_risk_class: best_delta,
            vega_by_risk_class: best_vega,
            curvature_by_risk_class: best_curvature,
            drc,
            rrao,
            binding_scenario: best_scenario,
            scenario_charges,
        })
    }
}

/// Builder for `FrtbSbaEngine`.
#[derive(Default)]
pub struct FrtbSbaEngineBuilder {
    scenarios: Option<Vec<CorrelationScenario>>,
    risk_classes: Option<Vec<FrtbRiskClass>>,
    reporting_currency: Option<Currency>,
    params: Option<super::params::FrtbParams>,
}

impl FrtbSbaEngineBuilder {
    /// Override correlation scenarios (default: Low, Medium, High).
    #[must_use]
    pub fn scenarios(mut self, s: Vec<CorrelationScenario>) -> Self {
        self.scenarios = Some(s);
        self
    }

    /// Restrict to specific risk classes (default: all).
    #[must_use]
    pub fn risk_classes(mut self, rc: Vec<FrtbRiskClass>) -> Self {
        self.risk_classes = Some(rc);
        self
    }

    /// Set reporting currency (default: USD).
    #[must_use]
    pub fn reporting_currency(mut self, ccy: Currency) -> Self {
        self.reporting_currency = Some(ccy);
        self
    }

    /// Override the FRTB parameter set (default: d457). Validated at `build()`.
    #[must_use]
    pub fn params(mut self, params: super::params::FrtbParams) -> Self {
        self.params = Some(params);
        self
    }

    /// Build the engine, validating configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if no scenarios or risk classes are configured.
    pub fn build(self) -> Result<FrtbSbaEngine> {
        let scenarios = self
            .scenarios
            .unwrap_or_else(|| CorrelationScenario::ALL.to_vec());
        let risk_classes = self
            .risk_classes
            .unwrap_or_else(|| FrtbRiskClass::ALL.to_vec());

        if scenarios.is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "FRTB SBA engine requires at least one correlation scenario".into(),
            ));
        }
        if risk_classes.is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "FRTB SBA engine requires at least one risk class".into(),
            ));
        }

        let params = self.params.unwrap_or_else(super::params::FrtbParams::d457);
        params.validate()?;

        Ok(FrtbSbaEngine {
            scenarios,
            risk_classes,
            reporting_currency: self.reporting_currency.unwrap_or(Currency::USD),
            params,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regulatory::frtb::types::*;

    // -----------------------------------------------------------------------
    // Builder tests
    // -----------------------------------------------------------------------

    #[test]
    fn builder_default_creates_engine() {
        let engine = FrtbSbaEngine::builder().build().expect("default build");
        assert_eq!(engine.scenarios.len(), 3);
        assert_eq!(engine.risk_classes.len(), FrtbRiskClass::ALL.len());
    }

    #[test]
    fn builder_rejects_empty_scenarios() {
        let err = FrtbSbaEngine::builder()
            .scenarios(vec![])
            .build()
            .expect_err("empty scenarios");
        assert!(err.to_string().contains("scenario"));
    }

    #[test]
    fn builder_rejects_empty_risk_classes() {
        let err = FrtbSbaEngine::builder()
            .risk_classes(vec![])
            .build()
            .expect_err("empty risk classes");
        assert!(err.to_string().contains("risk class"));
    }

    // -----------------------------------------------------------------------
    // FRTB delta: single GIRR factor
    // -----------------------------------------------------------------------

    #[test]
    fn single_girr_delta_produces_positive_charge() {
        let engine = FrtbSbaEngine::builder()
            .risk_classes(vec![FrtbRiskClass::Girr])
            .build()
            .expect("build");

        let mut sens = FrtbSensitivities::new(Currency::USD);
        sens.add_girr_delta(Currency::USD, "5Y", 100_000.0);

        let result = engine.calculate(&sens).expect("calculate");
        // RW for 5Y = 1.1, so WS = 100K * 1.1 = 110K.
        // Single factor in single bucket: charge = |WS| = 110K.
        assert!(
            result.total > 100_000.0,
            "GIRR delta charge should reflect risk weight: {}",
            result.total
        );
        assert!(
            result
                .delta_by_risk_class
                .contains_key(&FrtbRiskClass::Girr),
            "GIRR should be in delta breakdown"
        );
    }

    // -----------------------------------------------------------------------
    // FRTB equity delta
    // -----------------------------------------------------------------------

    #[test]
    fn equity_delta_charge_reflects_risk_weight() {
        let engine = FrtbSbaEngine::builder()
            .risk_classes(vec![FrtbRiskClass::Equity])
            .scenarios(vec![CorrelationScenario::Medium])
            .build()
            .expect("build");

        let mut sens = FrtbSensitivities::new(Currency::USD);
        // Bucket 12 (indices) has RW = 15.0.
        sens.add_equity_delta("SPX", 12, 100_000.0);

        let result = engine.calculate(&sens).expect("calculate");
        // WS = 100K * 15.0 = 1.5M.
        let expected = 100_000.0 * 15.0;
        let charge = result
            .delta_by_risk_class
            .get(&FrtbRiskClass::Equity)
            .copied()
            .unwrap_or(0.0);
        assert!(
            (charge - expected).abs() < 1.0,
            "single equity delta: expected {expected}, got {charge}"
        );
    }

    // -----------------------------------------------------------------------
    // FX delta
    // -----------------------------------------------------------------------

    #[test]
    fn fx_delta_uniform_weight() {
        let engine = FrtbSbaEngine::builder()
            .risk_classes(vec![FrtbRiskClass::Fx])
            .scenarios(vec![CorrelationScenario::Medium])
            .build()
            .expect("build");

        let mut sens = FrtbSensitivities::new(Currency::USD);
        sens.add_fx_delta(Currency::EUR, Currency::USD, 100_000.0);

        let result = engine.calculate(&sens).expect("calculate");
        // RW = 15.0, single factor: charge = 100K * 15 = 1.5M.
        let expected = 100_000.0 * 15.0;
        let charge = result
            .delta_by_risk_class
            .get(&FrtbRiskClass::Fx)
            .copied()
            .unwrap_or(0.0);
        assert!(
            (charge - expected).abs() < 1.0,
            "FX delta: expected {expected}, got {charge}"
        );
    }

    // -----------------------------------------------------------------------
    // Correlation scenario monotonicity
    // -----------------------------------------------------------------------

    #[test]
    fn scenario_charges_differ_for_multi_factor() {
        let engine = FrtbSbaEngine::builder().build().expect("build");

        let mut sens = FrtbSensitivities::new(Currency::USD);
        sens.add_girr_delta(Currency::USD, "5Y", 100_000.0);
        sens.add_girr_delta(Currency::EUR, "10Y", 80_000.0);

        let result = engine.calculate(&sens).expect("calculate");
        // With multiple factors, different scenarios should produce different charges.
        let low = result
            .scenario_charges
            .get(&CorrelationScenario::Low)
            .copied()
            .unwrap_or(0.0);
        let medium = result
            .scenario_charges
            .get(&CorrelationScenario::Medium)
            .copied()
            .unwrap_or(0.0);
        let high = result
            .scenario_charges
            .get(&CorrelationScenario::High)
            .copied()
            .unwrap_or(0.0);

        // All should be positive.
        assert!(low > 0.0, "low scenario charge positive");
        assert!(medium > 0.0, "medium scenario charge positive");
        assert!(high > 0.0, "high scenario charge positive");
    }

    // -----------------------------------------------------------------------
    // DRC
    // -----------------------------------------------------------------------

    #[test]
    fn drc_long_position_applies_risk_weight() {
        let engine = FrtbSbaEngine::builder().build().expect("build");

        let mut sens = FrtbSensitivities::new(Currency::USD);
        sens.drc_positions.push(DrcPosition {
            issuer: "ACME".to_string(),
            jtd_amount: 1_000_000.0,
            rating_bucket: 4, // BBB -> RW = 0.06 per MAR22.24
            sector: DrcSector::FinancialsCorporate,
            seniority: DrcSeniority::SeniorUnsecured,
            asset_type: DrcAssetType::Corporate,
            pnl_adjustment: 0.0,
        });

        let result = engine.calculate(&sens).expect("calculate");
        // DRC = LGD * JTD * RW = 0.75 * 1M * 0.06 = 45,000.
        let expected = 0.75 * 1_000_000.0 * 0.06;
        assert!(
            (result.drc - expected).abs() < 1.0,
            "DRC: expected {expected}, got {}",
            result.drc
        );
    }

    /// Two issuers in *different* sector buckets should not hedge each
    /// other. Previously HBR was computed globally, so a sovereign short
    /// could offset a corporate long even though MAR22.23 forbids it.
    #[test]
    fn drc_hedge_benefit_is_per_bucket() {
        let engine = FrtbSbaEngine::builder().build().expect("build");

        let mut sens = FrtbSensitivities::new(Currency::USD);
        // Long corporate bond (FinancialsCorporate bucket).
        sens.drc_positions.push(DrcPosition {
            issuer: "CORP_A".to_string(),
            jtd_amount: 1_000_000.0,
            rating_bucket: 4, // BBB, RW = 0.06
            sector: DrcSector::FinancialsCorporate,
            seniority: DrcSeniority::SeniorUnsecured,
            asset_type: DrcAssetType::Corporate,
            pnl_adjustment: 0.0,
        });
        // Short sovereign exposure (Sovereign bucket) — must NOT hedge
        // the corporate long.
        sens.drc_positions.push(DrcPosition {
            issuer: "SOV_A".to_string(),
            jtd_amount: -1_000_000.0,
            rating_bucket: 2, // AA, RW = 0.02
            sector: DrcSector::Sovereign,
            seniority: DrcSeniority::SeniorUnsecured,
            asset_type: DrcAssetType::Sovereign,
            pnl_adjustment: 0.0,
        });

        let result = engine.calculate(&sens).expect("calculate");
        // With per-bucket HBR: corporate bucket has only a long
        // (HBR_corp = 1, DRC_corp = 0.75 * 1M * 0.06 = 45k). Sovereign
        // bucket has only a short (HBR_sov = 0, DRC_sov = 0). Total = 45k.
        // Under the buggy global HBR, the short would offset the long
        // and total would be strictly less than 45k.
        let expected = 0.75 * 1_000_000.0 * 0.06;
        assert!(
            (result.drc - expected).abs() < 1.0,
            "per-bucket DRC: expected {expected}, got {} (buggy global-HBR \
             would give a smaller number because the sov short would offset \
             the corp long)",
            result.drc
        );
    }

    // -----------------------------------------------------------------------
    // RRAO
    // -----------------------------------------------------------------------

    #[test]
    fn rrao_exotic_applies_one_percent() {
        let engine = FrtbSbaEngine::builder().build().expect("build");

        let mut sens = FrtbSensitivities::new(Currency::USD);
        sens.rrao_exotic_notionals.push(RraoPosition {
            instrument_id: "EXOTIC_1".to_string(),
            notional: 10_000_000.0,
            is_exotic: true,
        });

        let result = engine.calculate(&sens).expect("calculate");
        let expected = 10_000_000.0 * 0.01;
        assert!(
            (result.rrao - expected).abs() < 1.0,
            "RRAO: expected {expected}, got {}",
            result.rrao
        );
    }

    #[test]
    fn rrao_other_applies_point_one_percent() {
        let engine = FrtbSbaEngine::builder().build().expect("build");

        let mut sens = FrtbSensitivities::new(Currency::USD);
        sens.rrao_exotic_notionals.push(RraoPosition {
            instrument_id: "GAP_1".to_string(),
            notional: 10_000_000.0,
            is_exotic: false,
        });

        let result = engine.calculate(&sens).expect("calculate");
        let expected = 10_000_000.0 * 0.001;
        assert!(
            (result.rrao - expected).abs() < 1.0,
            "RRAO: expected {expected}, got {}",
            result.rrao
        );
    }

    // -----------------------------------------------------------------------
    // CSR delta: name x tenor correlation factorisation
    // -----------------------------------------------------------------------

    /// Two CSR-non-sec sensitivities in the same bucket but at different
    /// tenors should aggregate with effective rho = rho_name * rho_tenor.
    /// Previously the code applied only rho_name (0.35), over-stating
    /// capital because it ignored the additional tenor offset (0.65).
    #[test]
    fn csr_intra_bucket_tenor_rho_is_applied() {
        let engine = FrtbSbaEngine::builder()
            .risk_classes(vec![FrtbRiskClass::CsrNonSec])
            .scenarios(vec![CorrelationScenario::Medium])
            .build()
            .expect("build");

        // Two issuer positions, same bucket (4 = basic materials), same name,
        // different tenors: name-identity rho = 1.0, but tenor rho = 0.65
        // kicks in.
        let mut sens_diff_tenor = FrtbSensitivities::new(Currency::USD);
        sens_diff_tenor.add_csr_nonsec_delta("ISSUER_A", 4, "1Y", 1_000_000.0);
        sens_diff_tenor.add_csr_nonsec_delta("ISSUER_A", 4, "5Y", 1_000_000.0);

        // Same name, same tenor: both factors = 1, full correlation.
        let mut sens_same_tenor = FrtbSensitivities::new(Currency::USD);
        sens_same_tenor.add_csr_nonsec_delta("ISSUER_A", 4, "1Y", 1_000_000.0);
        sens_same_tenor.add_csr_nonsec_delta("ISSUER_A", 4, "1Y", 1_000_000.0);

        let charge_diff = engine
            .calculate(&sens_diff_tenor)
            .expect("calc diff")
            .delta_by_risk_class
            .get(&FrtbRiskClass::CsrNonSec)
            .copied()
            .unwrap_or(0.0);
        let charge_same = engine
            .calculate(&sens_same_tenor)
            .expect("calc same")
            .delta_by_risk_class
            .get(&FrtbRiskClass::CsrNonSec)
            .copied()
            .unwrap_or(0.0);

        // Different-tenor charge must be strictly less than same-tenor
        // charge (otherwise the tenor correlation factor is being dropped).
        assert!(
            charge_diff < charge_same,
            "diff-tenor CSR charge ({charge_diff}) must be < same-tenor ({charge_same}) \
             because tenor rho < 1"
        );
    }

    // -----------------------------------------------------------------------
    // Empty sensitivities
    // -----------------------------------------------------------------------

    #[test]
    fn empty_sensitivities_zero_charge() {
        let engine = FrtbSbaEngine::builder().build().expect("build");
        let sens = FrtbSensitivities::new(Currency::USD);
        let result = engine.calculate(&sens).expect("calculate");
        assert!(
            result.total.abs() < 1e-10,
            "empty sensitivities: total = {}",
            result.total
        );
    }

    // -----------------------------------------------------------------------
    // Multi-risk-class portfolio
    // -----------------------------------------------------------------------

    #[test]
    fn multi_risk_class_total_exceeds_individual() {
        let engine = FrtbSbaEngine::builder()
            .scenarios(vec![CorrelationScenario::Medium])
            .build()
            .expect("build");

        let mut sens = FrtbSensitivities::new(Currency::USD);
        sens.add_girr_delta(Currency::USD, "5Y", 100_000.0);
        sens.add_equity_delta("AAPL", 1, 200_000.0);
        sens.add_fx_delta(Currency::EUR, Currency::USD, 50_000.0);

        let result = engine.calculate(&sens).expect("calculate");

        let girr_charge = result
            .delta_by_risk_class
            .get(&FrtbRiskClass::Girr)
            .copied()
            .unwrap_or(0.0);
        let equity_charge = result
            .delta_by_risk_class
            .get(&FrtbRiskClass::Equity)
            .copied()
            .unwrap_or(0.0);
        let fx_charge = result
            .delta_by_risk_class
            .get(&FrtbRiskClass::Fx)
            .copied()
            .unwrap_or(0.0);

        // Total should be the sum (FRTB SBA has no cross-risk-class diversification).
        let expected_total = girr_charge + equity_charge + fx_charge;
        assert!(
            (result.total - expected_total).abs() < 1.0,
            "multi-rc total: expected {expected_total}, got {}",
            result.total
        );
    }

    // -----------------------------------------------------------------------
    // Correlation scenario scaling
    // -----------------------------------------------------------------------

    #[test]
    fn correlation_scenario_scaling() {
        let low = CorrelationScenario::Low.scale_correlation(0.5);
        let medium = CorrelationScenario::Medium.scale_correlation(0.5);
        let high = CorrelationScenario::High.scale_correlation(0.5);

        assert!(
            (low - 0.375).abs() < 1e-10,
            "Low(0.5) = max(2*0.5-1, 0.75*0.5) = 0.375"
        );
        assert!((medium - 0.5).abs() < 1e-10, "Medium(0.5) = 0.5");
        assert!((high - 0.625).abs() < 1e-10, "High(0.5) = 1.25*0.5 = 0.625");
    }

    #[test]
    fn correlation_scenario_clamping() {
        // Low scenario: max(2*0.1 - 1, 0.75*0.1) = max(-0.8, 0.075) = 0.075.
        let low = CorrelationScenario::Low.scale_correlation(0.1);
        assert!((low - 0.075).abs() < 1e-10);

        // High scenario: min(1.25 * 0.9, 1.0) = min(1.125, 1.0) = 1.0.
        let high = CorrelationScenario::High.scale_correlation(0.9);
        assert!((high - 1.0).abs() < 1e-10);
    }

    // -----------------------------------------------------------------------
    // Vega charge
    // -----------------------------------------------------------------------

    #[test]
    fn girr_vega_single_factor() {
        let engine = FrtbSbaEngine::builder()
            .risk_classes(vec![FrtbRiskClass::Girr])
            .scenarios(vec![CorrelationScenario::Medium])
            .build()
            .expect("build");

        let mut sens = FrtbSensitivities::new(Currency::USD);
        sens.add_girr_vega(Currency::USD, "1Y", "5Y", 500_000.0);

        let result = engine.calculate(&sens).expect("calculate");
        // Vega RW = 1.00 after liquidity-horizon scaling. Single factor: charge = 500K.
        let expected = 500_000.0;
        let vega_charge = result
            .vega_by_risk_class
            .get(&FrtbRiskClass::Girr)
            .copied()
            .unwrap_or(0.0);
        assert!(
            (vega_charge - expected).abs() < 1.0,
            "GIRR vega: expected {expected}, got {vega_charge}"
        );
    }

    // -----------------------------------------------------------------------
    // Curvature charge
    // -----------------------------------------------------------------------

    #[test]
    fn girr_curvature_positive_shock() {
        let engine = FrtbSbaEngine::builder()
            .risk_classes(vec![FrtbRiskClass::Girr])
            .scenarios(vec![CorrelationScenario::Medium])
            .build()
            .expect("build");

        let mut sens = FrtbSensitivities::new(Currency::USD);
        // CVR_up = 50K, CVR_down = 30K. max(50K, 30K) = 50K. max(50K, 0) = 50K.
        sens.add_girr_curvature(Currency::USD, 50_000.0, 30_000.0);

        let result = engine.calculate(&sens).expect("calculate");
        let curv = result
            .curvature_by_risk_class
            .get(&FrtbRiskClass::Girr)
            .copied()
            .unwrap_or(0.0);
        assert!(
            (curv - 50_000.0).abs() < 1.0,
            "GIRR curvature: expected 50K, got {curv}"
        );
    }

    // -----------------------------------------------------------------------
    // GIRR tenor correlation
    // -----------------------------------------------------------------------

    #[test]
    fn girr_tenor_correlation_same_tenor() {
        use crate::regulatory::frtb::params::girr::girr_tenor_correlation;
        let rho = girr_tenor_correlation(5.0, 5.0);
        assert!((rho - 1.0).abs() < 1e-10, "same tenor => rho = 1.0");
    }

    #[test]
    fn girr_tenor_correlation_different_tenors() {
        use crate::regulatory::frtb::params::girr::girr_tenor_correlation;
        let rho = girr_tenor_correlation(5.0, 10.0);
        // exp(-0.03 * |5-10| / min(5,10)) = exp(-0.03 * 5/5) = exp(-0.03) ~ 0.9704
        let expected = (-0.03_f64).exp();
        assert!(
            (rho - expected).abs() < 1e-4,
            "5Y vs 10Y: expected {expected}, got {rho}"
        );
    }

    #[test]
    fn girr_tenor_correlation_floor() {
        use crate::regulatory::frtb::params::girr::girr_tenor_correlation;
        // 0.25Y vs 30Y: large diff/min ratio => floor = 0.40.
        let rho = girr_tenor_correlation(0.25, 30.0);
        assert!(
            (rho - 0.40).abs() < 1e-4,
            "0.25Y vs 30Y: expected 0.40 (floor), got {rho}"
        );
    }
}
