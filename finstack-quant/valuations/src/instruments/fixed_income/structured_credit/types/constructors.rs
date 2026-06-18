//! Constructor methods for StructuredCredit instruments.
//!
//! This module provides deal-type specific constructors that apply
//! appropriate defaults for ABS, CLO, CMBS, and RMBS instruments.

use super::{
    AssetPool, CreditFactors, CreditModelConfig, DealType, DefaultModelSpec, MarketConditions,
    Metadata, Overrides, PrepaymentModelSpec, RecoveryModelSpec, StructuredCredit, Tranche,
    TrancheCoupon, TrancheSeniority, TrancheStructure,
};
use crate::instruments::fixed_income::structured_credit::assumptions::{
    embedded_registry_or_panic, StructuredCreditAssumptionRegistry,
};
use crate::instruments::fixed_income::structured_credit::types::setup::DefaultAssumptions;
use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_core::Result;

use crate::instruments::common_impl::traits::Attributes;

/// Deal-specific configuration for constructor
pub(super) struct DealConfig {
    pub first_payment_date: Date,
    pub frequency: Tenor,
    pub prepayment_spec: PrepaymentModelSpec,
    pub default_spec: DefaultModelSpec,
    pub recovery_spec: RecoveryModelSpec,
    pub credit_factors: CreditFactors,
    pub deal_metadata: Metadata,
    pub behavior_overrides: Overrides,
}

/// Core instrument parameters shared across constructors
pub(super) struct InstrumentParams<'a> {
    pub pool: AssetPool,
    pub tranches: TrancheStructure,
    pub maturity: Date,
    pub discount_curve_id: &'a str,
}

impl StructuredCredit {
    /// Apply deal-type specific defaults to a builder.
    ///
    /// This method configures sensible defaults for payment frequency,
    /// behavioral models, and other deal-type specific parameters.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use finstack_quant_valuations::instruments::fixed_income::structured_credit::{DealType, StructuredCredit};
    ///
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// // Start from the canonical example deal and re-apply deal defaults explicitly.
    /// let base = StructuredCredit::example();
    /// let clo = StructuredCredit::apply_deal_defaults(
    ///     "MY_CLO",
    ///     DealType::CLO,
    ///     base.pool.clone(),
    ///     base.tranches.clone(),
    ///     base.closing_date,
    ///     base.maturity,
    ///     base.discount_curve_id.as_str(),
    /// );
    /// # let _ = clo;
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub fn apply_deal_defaults(
        id: impl Into<String>,
        deal_type: DealType,
        pool: AssetPool,
        tranches: TrancheStructure,
        closing_date: Date,
        maturity: Date,
        discount_curve_id: impl Into<String>,
    ) -> Self {
        match deal_type {
            DealType::CLO => Self::new_clo(
                id,
                pool,
                tranches,
                closing_date,
                maturity,
                discount_curve_id,
            ),
            DealType::CMBS => Self::new_cmbs(
                id,
                pool,
                tranches,
                closing_date,
                maturity,
                discount_curve_id,
            ),
            DealType::RMBS => Self::new_rmbs(
                id,
                pool,
                tranches,
                closing_date,
                maturity,
                discount_curve_id,
            ),
            _ => Self::new_abs(
                id,
                pool,
                tranches,
                closing_date,
                maturity,
                discount_curve_id,
            ), // Default to ABS
        }
    }

    /// Create a canonical example CLO structured credit deal with minimal components.
    ///
    /// This method is intended for testing, documentation examples, and quick prototyping.
    /// It creates a fully valid CLO deal with a single senior tranche and basic waterfall.
    ///
    /// # Panics
    ///
    /// Panics if the hard-coded example dates (2024-01-01, 2034-01-01) are invalid.
    /// This should never happen barring library bugs in the `time` crate.
    #[must_use]
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example() -> Self {
        use finstack_quant_core::currency::Currency;
        use time::Month;
        // Build a minimal pool (empty assets for example purposes)
        let pool = AssetPool::new("POOL-1", DealType::CLO, Currency::USD);
        // Build a simple tranche structure with single tranche 0-100%
        let tranche = Tranche::new(
            "CLONOTES-A",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::new(100_000_000.0, Currency::USD),
            TrancheCoupon::Fixed { rate: 0.06 },
            Date::from_calendar_date(2034, Month::January, 1).expect("Valid example date"),
        )
        .expect("Tranche build should not fail");
        let tranches = TrancheStructure::new(vec![tranche]).expect("TrancheStructure should build");
        let closing =
            Date::from_calendar_date(2024, Month::January, 1).expect("Valid example date");
        let legal = Date::from_calendar_date(2034, Month::January, 1).expect("Valid example date");
        StructuredCredit::new_clo("CLO-EXAMPLE", pool, tranches, closing, legal, "USD-OIS")
            .with_payment_calendar("nyse")
    }

    /// Internal helper to create structured credit with common fields
    pub(super) fn new_with_deal_config(
        id: impl Into<String>,
        deal_type: DealType,
        params: InstrumentParams,
        config: DealConfig,
        closing_date: Date,
    ) -> Self {
        let id_str = id.into();
        Self {
            id: InstrumentId::new(id_str),
            deal_type,
            pool: params.pool,
            tranches: params.tranches,
            closing_date,
            first_payment_date: config.first_payment_date,
            reinvestment_end_date: None,
            maturity: params.maturity,
            frequency: config.frequency,
            payment_calendar_id: None,
            payment_bdc: None,
            discount_curve_id: CurveId::new(params.discount_curve_id.to_string()),
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: Attributes::new(),
            credit_model: CreditModelConfig {
                prepayment_spec: config.prepayment_spec,
                default_spec: config.default_spec,
                recovery_spec: config.recovery_spec,
                stochastic_prepay_spec: None,
                stochastic_default_spec: None,
                correlation_structure: None,
            },
            market_conditions: MarketConditions::default(),
            credit_factors: config.credit_factors,
            deal_metadata: config.deal_metadata,
            behavior_overrides: config.behavior_overrides,
            default_assumptions: DefaultAssumptions::default(),
            // Hedge swaps default to empty
            hedge_swaps: Vec::new(),
            cleanup_call_pct: None,
            // No declarative waterfall rules by default.
            waterfall_rules: None,
        }
    }

    /// Create a new ABS instrument from its building blocks.
    ///
    #[allow(clippy::expect_used)] // Builder with valid default dates
    pub fn new_abs(
        id: impl Into<String>,
        pool: AssetPool,
        tranches: TrancheStructure,
        closing_date: Date,
        maturity: Date,
        discount_curve_id: impl Into<String>,
    ) -> Self {
        let disc_id_str = discount_curve_id.into();
        let mut inst = Self::new_with_deal_config(
            id,
            DealType::ABS,
            InstrumentParams {
                pool,
                tranches,
                maturity,
                discount_curve_id: &disc_id_str,
            },
            deal_config_from_registry("abs_auto_standard"),
            closing_date,
        );
        inst.default_assumptions = DefaultAssumptions::abs_auto_standard();
        inst
    }

    /// Create a new CLO instrument from its building blocks.
    ///
    #[allow(clippy::expect_used)] // Builder with valid default dates
    pub fn new_clo(
        id: impl Into<String>,
        pool: AssetPool,
        tranches: TrancheStructure,
        closing_date: Date,
        maturity: Date,
        discount_curve_id: impl Into<String>,
    ) -> Self {
        let disc_id_str = discount_curve_id.into();
        let mut inst = Self::new_with_deal_config(
            id,
            DealType::CLO,
            InstrumentParams {
                pool,
                tranches,
                maturity,
                discount_curve_id: &disc_id_str,
            },
            deal_config_from_registry("clo_standard"),
            closing_date,
        );
        inst.default_assumptions = DefaultAssumptions::clo_standard();
        inst
    }

    /// Create a new CMBS instrument from its building blocks.
    ///
    #[allow(clippy::expect_used)] // Builder with valid default dates
    pub fn new_cmbs(
        id: impl Into<String>,
        pool: AssetPool,
        tranches: TrancheStructure,
        closing_date: Date,
        maturity: Date,
        discount_curve_id: impl Into<String>,
    ) -> Self {
        let disc_id_str = discount_curve_id.into();
        let mut inst = Self::new_with_deal_config(
            id,
            DealType::CMBS,
            InstrumentParams {
                pool,
                tranches,
                maturity,
                discount_curve_id: &disc_id_str,
            },
            deal_config_from_registry("cmbs_standard"),
            closing_date,
        );
        inst.default_assumptions = DefaultAssumptions::cmbs_standard();
        inst
    }

    /// Create a new RMBS instrument from its building blocks.
    ///
    #[allow(clippy::expect_used)] // Builder with valid default dates
    pub fn new_rmbs(
        id: impl Into<String>,
        pool: AssetPool,
        tranches: TrancheStructure,
        closing_date: Date,
        maturity: Date,
        discount_curve_id: impl Into<String>,
    ) -> Self {
        let disc_id_str = discount_curve_id.into();
        let mut inst = Self::new_with_deal_config(
            id,
            DealType::RMBS,
            InstrumentParams {
                pool,
                tranches,
                maturity,
                discount_curve_id: &disc_id_str,
            },
            deal_config_from_registry("rmbs_standard"),
            closing_date,
        );
        inst.default_assumptions = DefaultAssumptions::rmbs_standard();
        inst
    }
}

#[allow(clippy::expect_used)]
fn deal_config_from_registry(profile_id: &str) -> DealConfig {
    let defaults = required_assumption(
        assumptions_registry().constructor_defaults(profile_id),
        "constructor defaults",
    );
    let month =
        time::Month::try_from(defaults.first_payment_month).expect("validated first payment month");
    DealConfig {
        first_payment_date: Date::from_calendar_date(2025, month, 1)
            .expect("valid structured-credit first payment date"),
        frequency: defaults.frequency,
        prepayment_spec: defaults.prepayment_spec,
        default_spec: defaults.default_spec,
        recovery_spec: defaults.recovery_spec,
        credit_factors: defaults.credit_factors,
        deal_metadata: Metadata::default(),
        behavior_overrides: Overrides::default(),
    }
}

fn assumptions_registry() -> &'static StructuredCreditAssumptionRegistry {
    embedded_registry_or_panic()
}

#[allow(clippy::expect_used)]
fn required_assumption<T>(result: Result<T>, _label: &str) -> T {
    result.expect("embedded structured-credit assumptions registry value should exist")
}
