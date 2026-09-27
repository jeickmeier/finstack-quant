//! Numerical pricing, expected-loss, and sensitivity helpers for CDS tranches.
//!
use super::config::CdsTranchePricer;
use crate::instruments::common_impl::traits::Instrument;
use crate::pricer::expect_inst;
use finstack_quant_core::market_data::context::MarketContext;

/// Result of detailed jump-to-default calculation.
///
/// Provides the distribution of JTD impacts across all portfolio constituents,
/// which is essential for worst-case risk management scenarios.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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

/// Registry pricer for CDS Tranche using Gaussian Copula model
pub(crate) struct SimpleCdsTrancheHazardPricer {
    model_key: crate::pricer::ModelKey,
}

impl SimpleCdsTrancheHazardPricer {
    /// Create new CDS tranche pricer with default hazard rate model
    pub(crate) fn new() -> Self {
        Self {
            model_key: crate::pricer::ModelKey::HazardRate,
        }
    }
}

impl Default for SimpleCdsTrancheHazardPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::pricer::Pricer for SimpleCdsTrancheHazardPricer {
    fn key(&self) -> crate::pricer::PricerKey {
        crate::pricer::PricerKey::new(crate::pricer::InstrumentType::CdsTranche, self.model_key)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<crate::results::ValuationResult, crate::pricer::PricingError> {
        use crate::instruments::common_impl::traits::Instrument;

        // Type-safe downcasting
        let cds_tranche = expect_inst::<
            crate::instruments::credit_derivatives::cds_tranche::CdsTranche,
        >(instrument, crate::pricer::InstrumentType::CdsTranche)?;

        // Use the provided as_of date for valuation
        // Compute present value using the engine
        let pv = CdsTranchePricer::new()
            .price_tranche(cds_tranche, market, as_of)
            .map_err(|e| {
                crate::pricer::PricingError::model_failure_with_context(
                    e.to_string(),
                    crate::pricer::PricingErrorContext::default(),
                )
            })?;

        Ok(
            crate::results::ValuationResult::stamped(cds_tranche.id(), as_of, pv).with_details(
                crate::results::ValuationDetails::CreditDerivative(
                    crate::results::CreditDerivativeValuationDetails {
                        model_key: self.model_key,
                        integration_method: Some("isda_standard_model".to_string()),
                    },
                ),
            ),
        )
    }
}
