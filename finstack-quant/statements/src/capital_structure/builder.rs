//! Builder API Extensions for Capital Structure
//!
//! This module provides fluent builder methods for adding capital structure
//! instruments to a financial model.

use crate::builder::ModelBuilder;
use crate::types::{CapitalStructureSpec, DebtInstrumentSpec, FinancialStatementInstrument};

/// Helper to ensure capital structure exists and return mutable reference.
///
/// Returns a mutable reference to the capital structure spec, creating an empty
/// instance if one is not already present. Operates directly on the
/// `Option<CapitalStructureSpec>` field so it is not generic over the
/// builder's type-state (the capital structure is state-independent).
fn ensure_capital_structure(cs: &mut Option<CapitalStructureSpec>) -> &mut CapitalStructureSpec {
    cs.get_or_insert_with(|| CapitalStructureSpec {
        debt_instruments: vec![],
        meta: indexmap::IndexMap::new(),
        reporting_currency: None,
        fx_policy: None,
        waterfall: None,
    })
}

/// Append a typed debt entry to the capital structure.
fn push_debt(
    cs: &mut Option<CapitalStructureSpec>,
    id: String,
    spec: FinancialStatementInstrument,
) {
    ensure_capital_structure(cs)
        .debt_instruments
        .push(DebtInstrumentSpec { id, spec });
}

impl ModelBuilder {
    /// Add a typed debt instrument.
    ///
    /// This is the one way an instrument enters the capital structure.
    /// Construct bonds, swaps and loans through their canonical Valuations
    /// APIs, with fully authored terms or a deliberately selected dated
    /// convention; the instrument keeps its resolved terms and contractual
    /// schedule in the model.
    ///
    /// # Example
    /// ```no_run
    /// use finstack_quant_statements::builder::ModelBuilder;
    /// use finstack_quant_statements::types::FinancialStatementInstrument;
    ///
    /// # fn instrument() -> FinancialStatementInstrument { unimplemented!() }
    /// let builder = {
    ///     let mut model_builder = ModelBuilder::new("cs-model");
    ///     model_builder.add_debt("RCF-A", instrument());
    ///     model_builder
    /// };
    /// # let _ = builder;
    /// ```
    ///
    /// # Arguments
    ///
    /// * `id` - Unique instrument identifier stored on the capital-structure debt entry
    /// * `spec` - Typed bond, loan, revolver, convertible, swap, cap/floor, or swaption payload
    pub fn add_debt(
        &mut self,
        id: impl Into<String>,
        spec: FinancialStatementInstrument,
    ) -> &mut Self {
        push_debt(&mut self.capital_structure, id.into(), spec);
        self
    }

    /// Set an explicit reporting currency for capital-structure totals.
    ///
    /// # Arguments
    ///
    /// * `currency` - Currency of the reported capital-structure totals;
    ///   evaluation converts instrument cashflows using the configured FX policy.
    pub fn reporting_currency(
        &mut self,
        currency: finstack_quant_core::currency::Currency,
    ) -> &mut Self {
        ensure_capital_structure(&mut self.capital_structure).reporting_currency = Some(currency);
        self
    }

    /// Set the FX conversion policy used when converting capital-structure cashflows.
    ///
    /// Already-aggregated `cs.*` cash items and balances convert on the
    /// inclusive period-end date with this policy. There is no default: a model
    /// whose capital structure needs a cross-currency conversion fails until
    /// the policy is set (record CORE-CONVENTIONS-013, resolution O9).
    ///
    /// # Arguments
    ///
    /// * `policy` - How each period's native-currency totals are converted
    ///   into the reporting currency. `PeriodEnd`, the only policy, reads the
    ///   rate of the inclusive period-close date. It must be set before
    ///   instruments of more than one currency are consolidated.
    pub fn fx_policy(&mut self, policy: crate::types::CapitalFxPolicy) -> &mut Self {
        ensure_capital_structure(&mut self.capital_structure).fx_policy = Some(policy);
        self
    }

    /// Configure waterfall specification for dynamic cash flow allocation.
    ///
    /// # Arguments
    ///
    /// * `waterfall_spec` - Priority-of-payments, pre-waterfall cash node,
    ///   optional ECF sweep / PIK toggle, payment classes, and optional
    ///   mandatory / voluntary prepayment requests.
    ///
    /// # Example
    /// ```
    /// use finstack_quant_statements::capital_structure::{
    ///     EcfSweepSpec, EcfTerm, FacilitySelection, PaymentPriority, WaterfallSpec,
    /// };
    /// use finstack_quant_statements::builder::ModelBuilder;
    ///
    /// let input = |expression: &str| EcfTerm::Input {
    ///     expression: expression.to_string(),
    /// };
    /// let waterfall = WaterfallSpec {
    ///     priority_of_payments: vec![
    ///         PaymentPriority::Fees,
    ///         PaymentPriority::Interest,
    ///         PaymentPriority::Amortization,
    ///         PaymentPriority::Sweep,
    ///         PaymentPriority::Equity,
    ///     ],
    ///     ecf_sweep: Some(EcfSweepSpec {
    ///         cash_measure: "ebitda".to_string(),
    ///         deductions: vec![
    ///             input("taxes"),
    ///             input("capex"),
    ///             input("wc_change"),
    ///             EcfTerm::PaidCashInterest,
    ///             EcfTerm::PaidScheduledPrincipal,
    ///         ],
    ///         credits: Vec::new(),
    ///         sweep_percentage: 0.5, // 50% sweep
    ///         facilities: FacilitySelection::Named(vec!["TL-A".to_string()]),
    ///     }),
    ///     ..WaterfallSpec::default()
    /// };
    ///
    /// let builder = {
    ///     let mut model_builder = ModelBuilder::new("cs-model");
    ///     model_builder.waterfall(waterfall);
    ///     model_builder
    /// };
    /// # let _ = builder;
    /// ```
    pub fn waterfall(
        &mut self,
        waterfall_spec: crate::capital_structure::WaterfallSpec,
    ) -> &mut Self {
        ensure_capital_structure(&mut self.capital_structure).waterfall = Some(waterfall_spec);
        self
    }
}
