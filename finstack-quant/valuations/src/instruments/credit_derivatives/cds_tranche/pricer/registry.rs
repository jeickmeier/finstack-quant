use super::config::CDSTranchePricer;
use crate::instruments::common_impl::traits::Instrument;
use finstack_quant_core::market_data::context::MarketContext;

/// Result of detailed jump-to-default calculation.
///
/// Provides the distribution of JTD impacts across all portfolio constituents,
/// which is essential for worst-case risk management scenarios.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct JumpToDefaultResult {
    /// Minimum JTD impact across all names (best case)
    pub min: f64,
    /// Maximum JTD impact across all names (worst case for risk)
    pub max: f64,
    /// Average JTD impact across all names
    pub average: f64,
    /// Number of names that would impact this tranche
    pub count: usize,
}

impl JumpToDefaultResult {
    /// Check if any names would impact this tranche
    #[inline]
    pub fn has_impact(&self) -> bool {
        self.count > 0
    }

    /// Get the range of impacts (max - min)
    #[inline]
    pub fn impact_range(&self) -> f64 {
        self.max - self.min
    }
}

// ========================= REGISTRY PRICER =========================

/// Registry pricer for CDS Tranche using Gaussian Copula model
pub(crate) struct SimpleCDSTrancheHazardPricer {
    model_key: crate::pricer::ModelKey,
}

impl SimpleCDSTrancheHazardPricer {
    /// Create new CDS tranche pricer with default hazard rate model
    pub(crate) fn new() -> Self {
        Self {
            model_key: crate::pricer::ModelKey::HazardRate,
        }
    }
}

impl Default for SimpleCDSTrancheHazardPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::pricer::Pricer for SimpleCDSTrancheHazardPricer {
    fn key(&self) -> crate::pricer::PricerKey {
        crate::pricer::PricerKey::new(crate::pricer::InstrumentType::CDSTranche, self.model_key)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<crate::results::ValuationResult, crate::pricer::PricingError> {
        use crate::instruments::common_impl::traits::Instrument;

        // Type-safe downcasting
        let cds_tranche = instrument
            .as_any()
            .downcast_ref::<crate::instruments::credit_derivatives::cds_tranche::CDSTranche>()
            .ok_or_else(|| {
                crate::pricer::PricingError::type_mismatch(
                    crate::pricer::InstrumentType::CDSTranche,
                    instrument.key(),
                )
            })?;

        // Use the provided as_of date for valuation
        // Compute present value using the engine
        let pv = CDSTranchePricer::new()
            .price_tranche(cds_tranche, market, as_of)
            .map_err(|e| {
                crate::pricer::PricingError::model_failure_with_context(
                    e.to_string(),
                    crate::pricer::PricingErrorContext::default(),
                )
            })?;

        // Return stamped result
        Ok(
            crate::results::ValuationResult::stamped(cds_tranche.id(), as_of, pv).with_details(
                crate::results::ValuationDetails::CreditDerivative(
                    crate::results::CreditDerivativeValuationDetails {
                        model_key: format!("{:?}", self.model_key),
                        integration_method: Some("isda_standard_model".to_string()),
                    },
                ),
            ),
        )
    }
}
