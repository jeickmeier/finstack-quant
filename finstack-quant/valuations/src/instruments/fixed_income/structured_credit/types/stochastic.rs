//! Stochastic configuration helpers for StructuredCredit.
//!
//! This module provides methods to enable and configure stochastic
//! prepayment and default modeling for structured credit instruments.

use super::{DealType, StructuredCredit};
use crate::cashflow::builder::PrepaymentModelSpec;
use crate::instruments::fixed_income::structured_credit::pricing::{
    CorrelationStructure, StochasticDefaultSpec, StochasticPrepaySpec,
};

impl StructuredCredit {
    // =========================================================================
    // Stochastic configuration helpers
    // =========================================================================

    /// Check if stochastic modeling is enabled.
    ///
    /// Returns true if any stochastic specification is set.
    pub fn is_stochastic(&self) -> bool {
        self.credit_model.stochastic_prepay_spec.is_some()
            || self.credit_model.stochastic_default_spec.is_some()
            || self.credit_model.correlation_structure.is_some()
    }

    /// Enable stochastic prepayment modeling.
    ///
    /// # Arguments
    /// * `spec` - Stochastic prepayment specification
    ///
    /// # Example
    /// ```text
    /// use finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCredit;
    /// use finstack_quant_valuations::instruments::fixed_income::structured_credit::pricing::StochasticPrepaySpec;
    ///
    /// let mut clo = StructuredCredit::example();
    /// clo.with_stochastic_prepay(StochasticPrepaySpec::clo_standard());
    /// ```
    pub fn with_stochastic_prepay(&mut self, spec: StochasticPrepaySpec) -> &mut Self {
        self.credit_model.stochastic_prepay_spec = Some(spec);
        self
    }

    /// Enable stochastic default modeling.
    ///
    /// # Arguments
    /// * `spec` - Stochastic default specification
    pub fn with_stochastic_default(&mut self, spec: StochasticDefaultSpec) -> &mut Self {
        self.credit_model.stochastic_default_spec = Some(spec);
        self
    }

    /// Set correlation structure for stochastic modeling.
    ///
    /// # Arguments
    /// * `structure` - Correlation structure specification
    pub fn with_correlation(&mut self, structure: CorrelationStructure) -> &mut Self {
        self.credit_model.correlation_structure = Some(structure);
        self
    }

    /// Enable full stochastic modeling with default calibrations.
    ///
    /// Applies deal-type-appropriate stochastic models:
    /// - RMBS: Agency prepay model, low asset correlation
    /// - CLO: Corporate default correlation, sectored structure
    /// - CMBS: Moderate correlation, property-type focused
    /// - ABS: Low correlation, consumer-focused
    pub fn enable_stochastic_defaults(&mut self) -> &mut Self {
        let (prepay, default, corr) = match self.deal_type {
            DealType::RMBS => (
                StochasticPrepaySpec::rmbs_agency(if self.market_conditions.refi_rate > 0.0 {
                    self.market_conditions.refi_rate
                } else {
                    0.045 // Default mortgage rate
                }),
                StochasticDefaultSpec::rmbs_standard(),
                CorrelationStructure::rmbs_standard(),
            ),
            DealType::CLO | DealType::CBO => (
                StochasticPrepaySpec::clo_standard(),
                StochasticDefaultSpec::clo_standard(),
                CorrelationStructure::clo_standard(),
            ),
            DealType::CMBS => (
                // CMBS has minimal prepayment due to lockout/defeasance
                StochasticPrepaySpec::deterministic(PrepaymentModelSpec::constant_cpr(0.02)),
                StochasticDefaultSpec::gaussian_copula(0.02, 0.20),
                CorrelationStructure::cmbs_standard(),
            ),
            DealType::ABS | DealType::Auto | DealType::Card => (
                StochasticPrepaySpec::factor_correlated(
                    self.credit_model.prepayment_spec.clone(),
                    0.30,
                    0.15,
                ),
                StochasticDefaultSpec::gaussian_copula(self.credit_model.default_spec.cdr, 0.10),
                CorrelationStructure::abs_auto_standard(),
            ),
        };

        self.credit_model.stochastic_prepay_spec = Some(prepay);
        self.credit_model.stochastic_default_spec = Some(default);
        self.credit_model.correlation_structure = Some(corr);
        self
    }

    /// Clear stochastic specifications, reverting to deterministic pricing.
    pub fn disable_stochastic(&mut self) -> &mut Self {
        self.credit_model.stochastic_prepay_spec = None;
        self.credit_model.stochastic_default_spec = None;
        self.credit_model.correlation_structure = None;
        self
    }
}
