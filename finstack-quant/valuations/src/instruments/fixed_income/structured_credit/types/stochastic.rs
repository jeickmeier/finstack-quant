//! Stochastic configuration helpers for StructuredCredit.
//!
//! This module provides methods to enable and configure stochastic
//! prepayment and default modeling for structured credit instruments.

use super::{DealType, StructuredCredit};
use crate::cashflow::builder::PrepaymentModelSpec;
use crate::instruments::fixed_income::structured_credit::pricing::stochastic::calibrations::{
    abs_auto_correlation_structure, clo_correlation_structure, clo_default_spec, clo_prepay_spec,
    cmbs_correlation_structure, rmbs_correlation_structure, rmbs_default_spec, rmbs_prepay_spec,
};
use finstack_quant_core::Result;
use finstack_quant_models::credit::pool::{
    CorrelationStructure, StochasticDefaultSpec, StochasticPrepaySpec,
};

impl StructuredCredit {
    // Stochastic configuration helpers

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
    /// use finstack_quant_models::credit::pool::StochasticPrepaySpec;
    ///
    /// let mut clo = StructuredCredit::example();
    /// clo.with_stochastic_prepay(StochasticPrepaySpec::factor_correlated(
    ///     finstack_quant_cashflows::builder::PrepaymentModelSpec::constant_cpr(0.15),
    ///     0.35,
    ///     0.25,
    /// ));
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
    ///
    /// # Errors
    ///
    /// Returns an error if an embedded deal-type correlation preset is invalid.
    pub fn enable_stochastic(&mut self) -> Result<&mut Self> {
        let (prepay, default, corr) = match self.deal_type {
            DealType::Rmbs => (
                // Pool WAC is the coupon side of the Richard-Roll incentive;
                // market rate arrives via `tree_config.market_refi_rate`.
                rmbs_prepay_spec(self.pool.wac()),
                rmbs_default_spec(),
                rmbs_correlation_structure()?,
            ),
            DealType::Clo | DealType::Cbo => (
                clo_prepay_spec(),
                clo_default_spec(),
                clo_correlation_structure()?,
            ),
            DealType::Cmbs => (
                // CMBS has minimal prepayment due to lockout/defeasance
                StochasticPrepaySpec::deterministic(PrepaymentModelSpec::constant_cpr(0.02)),
                StochasticDefaultSpec::gaussian_copula(0.02, 0.20),
                cmbs_correlation_structure()?,
            ),
            DealType::Abs | DealType::Auto | DealType::Card => (
                StochasticPrepaySpec::factor_correlated(
                    self.credit_model.prepayment_spec.clone(),
                    0.30,
                    0.15,
                ),
                StochasticDefaultSpec::gaussian_copula(self.credit_model.default_spec.cdr, 0.10),
                abs_auto_correlation_structure()?,
            ),
        };

        self.credit_model.stochastic_prepay_spec = Some(prepay);
        self.credit_model.stochastic_default_spec = Some(default);
        self.credit_model.correlation_structure = Some(corr);
        Ok(self)
    }

    /// Clear stochastic specifications, reverting to deterministic pricing.
    pub fn disable_stochastic(&mut self) -> &mut Self {
        self.credit_model.stochastic_prepay_spec = None;
        self.credit_model.stochastic_default_spec = None;
        self.credit_model.correlation_structure = None;
        self
    }
}
