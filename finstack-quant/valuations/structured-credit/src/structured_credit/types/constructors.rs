//! Constructor methods for StructuredCredit instruments.
//!
//! This module provides deal-type specific constructors that apply
//! appropriate defaults for ABS, CLO, CMBS, and RMBS instruments.

use super::{
    AssetPool, CreditModelConfig, DealType, MarketConditions, Metadata, PoolAsset,
    StructuredCredit, Tranche, TrancheSeniority, TrancheStructure,
};
use crate::structured_credit::assumptions::{embedded_registry_or_panic, required_assumption};
use finstack_quant_cashflows::builder::RateSpec;
use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::dates::{Date, DateExt, DayCount, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CalendarId, CurveId, InstrumentId};

use finstack_quant_core::types::Attributes;
use finstack_quant_valuations_core::instruments::Instrument;

/// Deal-specific configuration for constructor
pub(super) struct DealConfig {
    pub first_payment_date: Date,
    pub frequency: Tenor,
    pub prepayment_spec: PrepaymentModelSpec,
    pub default_spec: DefaultModelSpec,
    pub recovery_spec: RecoveryModelSpec,
    pub deal_metadata: Metadata,
}

/// Inputs of [`StructuredCredit::new_standard`], the validated deal-type
/// constructor the Python and WASM bindings call.
///
/// The fields carry the same meaning as the arguments of
/// [`StructuredCredit::new_abs`]; `calendar_id` is the optional payment
/// calendar applied before validation.
#[derive(Debug, Clone)]
pub struct StandardDealParams {
    /// Stable deal identifier used for pricing results and risk labels.
    pub id: InstrumentId,
    /// Current collateral balances and contractual asset terms, in the pool currency.
    pub pool: AssetPool,
    /// Capital structure, current note balances and coupon terms.
    pub tranches: TrancheStructure,
    /// Contractual deal inception; the first accrual boundary is one registry
    /// payment period later (capped at maturity).
    pub closing_date: Date,
    /// Legal final date, strictly after closing.
    pub maturity: Date,
    /// Market-context discount curve used for tranche PV.
    pub discount_curve_id: CurveId,
    /// Holiday calendar for payment-date adjustment; `None` leaves payment
    /// dates unadjusted.
    pub calendar_id: Option<CalendarId>,
}

/// Core instrument parameters shared across constructors
pub(super) struct InstrumentParams<'a> {
    pub pool: AssetPool,
    pub tranches: TrancheStructure,
    pub maturity: Date,
    pub discount_curve_id: &'a str,
}

impl StructuredCredit {
    /// Create a canonical, priceable example CLO structured-credit deal.
    ///
    /// This method is intended for testing, documentation examples, and quick prototyping.
    /// It creates a fully valid CLO deal with one fixed-rate collateral asset,
    /// a matching senior tranche, and a basic waterfall.
    ///
    /// # Errors
    ///
    /// Returns an error if the hard-coded example pool or tranches fail validation.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::currency::Currency;
        let closing = time::macros::date!(2024 - 01 - 01);
        let legal = time::macros::date!(2034 - 01 - 01);
        let mut pool = AssetPool::new("POOL-1", DealType::Clo, Currency::USD);
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "COLLATERAL-1",
            Money::from((100_000_000_i64, Currency::USD)),
            0.07,
            legal,
            // ACT/360 matches the CLO/leveraged-loan market convention and
            // the deal-level day count assigned in `raw_cashflow_schedule`.
            DayCount::Act360,
        ));
        let tranche = Tranche::new(
            "CLONOTES-A",
            0.0,
            100.0,
            TrancheSeniority::Senior,
            Money::from((100_000_000_i64, Currency::USD)),
            RateSpec::Fixed { rate: 0.06 },
            legal,
        )?;
        let tranches = TrancheStructure::new(vec![tranche])?;
        Ok(
            StructuredCredit::new_clo("CLO-EXAMPLE", pool, tranches, closing, legal, "USD-OIS")?
                .with_calendar_id("nyse"),
        )
    }

    /// Internal helper to create structured credit with common fields
    pub(super) fn new_with_deal_config(
        id: impl Into<InstrumentId>,
        deal_type: DealType,
        params: InstrumentParams,
        config: DealConfig,
        closing_date: Date,
    ) -> Self {
        Self {
            id: id.into(),
            deal_type,
            // Senior fees are opt-in via `fees` / `with_standard_fees`.
            fees: None,
            // OC/IC triggers are opt-in via `coverage_triggers`.
            coverage_triggers: Vec::new(),
            pool: params.pool,
            tranches: params.tranches,
            closing_date,
            first_payment_date: config.first_payment_date.min(params.maturity),
            maturity: params.maturity,
            quote_settlement_date: None,
            frequency: config.frequency,
            calendar_id: None,
            business_day_convention: None,
            discount_curve_id: CurveId::new(params.discount_curve_id.to_string()),
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
            credit_model: CreditModelConfig {
                prepayment_spec: config.prepayment_spec,
                default_spec: config.default_spec,
                recovery_spec: config.recovery_spec,
                stochastic_prepay_spec: None,
                stochastic_default_spec: None,
                stochastic_recovery_spec: None,
                correlation_structure: None,
                delinquency: None,
                card: None,
            },
            market_conditions: MarketConditions::default(),
            deal_metadata: config.deal_metadata,
            // Hedge swaps default to empty
            hedge_swaps: Vec::new(),
            cleanup_call_decimal: None,
            // Deal-type market convention; see `LossAllocationPolicy::default_for`.
            loss_allocation: None,
            loss_recognition: None,
            // Deal-type convention; see `effective_principal_covers_senior_interest`.
            principal_covers_senior_interest: None,
            // Par-OC unless rules are attached; see `CoverageRules::clo_standard`.
            coverage_rules: None,
            call_assumption: None,
            tranche_draws: Vec::new(),
            tranche_readvance: None,
            liquidation_price_pct: None,
            // No declarative waterfall rules by default.
            waterfall_rules: None,
            // Template waterfall by default; custom via `waterfall`.
            waterfall: None,
        }
    }

    /// Create a validated deal of a standard deal type with its registry
    /// defaults and an optional payment calendar.
    ///
    /// Dispatches to [`Self::new_abs`], [`Self::new_clo`], [`Self::new_cmbs`] or
    /// [`Self::new_rmbs`], applies `params.calendar_id`, then runs
    /// [`Instrument::validate_for_pricing`] so the returned deal is ready to price.
    ///
    /// # Arguments
    ///
    /// * `deal_type` - Deal family selecting the registry constructor profile;
    ///   one of [`DealType::Abs`], [`DealType::Clo`], [`DealType::Cmbs`] or
    ///   [`DealType::Rmbs`].
    /// * `params` - Deal identifier, collateral pool, tranches, closing and
    ///   maturity dates, discount curve and optional payment calendar.
    ///
    /// # Errors
    ///
    /// Returns a validation error if `deal_type` has no standard constructor
    /// (CBO, auto, card), if the first registry payment period exceeds the
    /// supported calendar range, or if the deal fails pricing validation.
    pub fn new_standard(
        deal_type: DealType,
        params: StandardDealParams,
    ) -> finstack_quant_core::Result<Self> {
        let StandardDealParams {
            id,
            pool,
            tranches,
            closing_date,
            maturity,
            discount_curve_id,
            calendar_id,
        } = params;
        let mut deal = match deal_type {
            DealType::Abs => Self::new_abs(
                id,
                pool,
                tranches,
                closing_date,
                maturity,
                discount_curve_id,
            ),
            DealType::Clo => Self::new_clo(
                id,
                pool,
                tranches,
                closing_date,
                maturity,
                discount_curve_id,
            ),
            DealType::Cmbs => Self::new_cmbs(
                id,
                pool,
                tranches,
                closing_date,
                maturity,
                discount_curve_id,
            ),
            DealType::Rmbs => Self::new_rmbs(
                id,
                pool,
                tranches,
                closing_date,
                maturity,
                discount_curve_id,
            ),
            other => Err(finstack_quant_core::Error::Validation(format!(
                "deal type {other:?} has no standard constructor; use the StructuredCredit builder"
            ))),
        }?;
        deal.calendar_id = calendar_id;
        deal.validate_for_pricing()?;
        Ok(deal)
    }

    /// Create a new ABS instrument from its building blocks.
    ///
    /// # Arguments
    ///
    /// * `id` - Stable deal identifier used for pricing results and risk labels.
    /// * `pool` - Current collateral balances and contractual asset terms; supply
    ///   either individual assets or representative lines, in the pool currency.
    /// * `tranches` - Capital structure, current note balances and coupon terms.
    /// * `closing_date` - Contractual deal inception; the first accrual boundary
    ///   is one registry payment period later (capped at maturity). Payment dates
    ///   are adjusted by the configured calendar during schedule generation.
    /// * `maturity` - Legal final date, strictly after closing.
    /// * `discount_curve_id` - Market-context discount curve used for tranche PV.
    ///
    /// # Errors
    ///
    /// Returns a validation error if the first registry payment period exceeds the supported calendar range.
    pub fn new_abs(
        id: impl Into<InstrumentId>,
        pool: AssetPool,
        tranches: TrancheStructure,
        closing_date: Date,
        maturity: Date,
        discount_curve_id: impl Into<CurveId>,
    ) -> finstack_quant_core::Result<Self> {
        let disc_id: CurveId = discount_curve_id.into();
        Ok(Self::new_with_deal_config(
            id,
            DealType::Abs,
            InstrumentParams {
                pool,
                tranches,
                maturity,
                discount_curve_id: disc_id.as_str(),
            },
            deal_config_from_registry("abs_auto_standard", closing_date)?,
            closing_date,
        ))
    }

    /// Create a new CLO instrument from its building blocks.
    ///
    /// # Arguments
    ///
    /// * `id` - Stable deal identifier used for pricing results and risk labels.
    /// * `pool` - Current collateral balances and contractual asset terms; supply
    ///   either individual assets or representative lines, in the pool currency.
    /// * `tranches` - Capital structure, current note balances and coupon terms.
    /// * `closing_date` - Contractual deal inception; the first accrual boundary
    ///   is one registry payment period later (capped at maturity). Payment dates
    ///   are adjusted by the configured calendar during schedule generation.
    /// * `maturity` - Legal final date, strictly after closing.
    /// * `discount_curve_id` - Market-context discount curve used for tranche PV.
    ///
    /// # Errors
    ///
    /// Returns a validation error if the first registry payment period exceeds the supported calendar range.
    pub fn new_clo(
        id: impl Into<InstrumentId>,
        pool: AssetPool,
        tranches: TrancheStructure,
        closing_date: Date,
        maturity: Date,
        discount_curve_id: impl Into<CurveId>,
    ) -> finstack_quant_core::Result<Self> {
        let disc_id: CurveId = discount_curve_id.into();
        Ok(Self::new_with_deal_config(
            id,
            DealType::Clo,
            InstrumentParams {
                pool,
                tranches,
                maturity,
                discount_curve_id: disc_id.as_str(),
            },
            deal_config_from_registry("clo_standard", closing_date)?,
            closing_date,
        ))
    }

    /// Create a new CMBS instrument from its building blocks.
    ///
    /// # Arguments
    ///
    /// * `id` - Stable deal identifier used for pricing results and risk labels.
    /// * `pool` - Current collateral balances and contractual asset terms; supply
    ///   either individual assets or representative lines, in the pool currency.
    /// * `tranches` - Capital structure, current note balances and coupon terms.
    /// * `closing_date` - Contractual deal inception; the first accrual boundary
    ///   is one registry payment period later (capped at maturity). Payment dates
    ///   are adjusted by the configured calendar during schedule generation.
    /// * `maturity` - Legal final date, strictly after closing.
    /// * `discount_curve_id` - Market-context discount curve used for tranche PV.
    ///
    /// # Errors
    ///
    /// Returns a validation error if the first registry payment period exceeds the supported calendar range.
    pub fn new_cmbs(
        id: impl Into<InstrumentId>,
        pool: AssetPool,
        tranches: TrancheStructure,
        closing_date: Date,
        maturity: Date,
        discount_curve_id: impl Into<CurveId>,
    ) -> finstack_quant_core::Result<Self> {
        let disc_id: CurveId = discount_curve_id.into();
        Ok(Self::new_with_deal_config(
            id,
            DealType::Cmbs,
            InstrumentParams {
                pool,
                tranches,
                maturity,
                discount_curve_id: disc_id.as_str(),
            },
            deal_config_from_registry("cmbs_standard", closing_date)?,
            closing_date,
        ))
    }

    /// Create a new RMBS instrument from its building blocks.
    ///
    /// # Arguments
    ///
    /// * `id` - Stable deal identifier used for pricing results and risk labels.
    /// * `pool` - Current collateral balances and contractual asset terms; supply
    ///   either individual assets or representative lines, in the pool currency.
    /// * `tranches` - Capital structure, current note balances and coupon terms.
    /// * `closing_date` - Contractual deal inception; the first accrual boundary
    ///   is one registry payment period later (capped at maturity). Payment dates
    ///   are adjusted by the configured calendar during schedule generation.
    /// * `maturity` - Legal final date, strictly after closing.
    /// * `discount_curve_id` - Market-context discount curve used for tranche PV.
    ///
    /// # Errors
    ///
    /// Returns a validation error if the first registry payment period exceeds the supported calendar range.
    pub fn new_rmbs(
        id: impl Into<InstrumentId>,
        pool: AssetPool,
        tranches: TrancheStructure,
        closing_date: Date,
        maturity: Date,
        discount_curve_id: impl Into<CurveId>,
    ) -> finstack_quant_core::Result<Self> {
        let disc_id: CurveId = discount_curve_id.into();
        Ok(Self::new_with_deal_config(
            id,
            DealType::Rmbs,
            InstrumentParams {
                pool,
                tranches,
                maturity,
                discount_curve_id: disc_id.as_str(),
            },
            deal_config_from_registry("rmbs_standard", closing_date)?,
            closing_date,
        ))
    }
}

#[allow(clippy::expect_used)]
fn deal_config_from_registry(
    profile_id: &str,
    closing_date: Date,
) -> finstack_quant_core::Result<DealConfig> {
    let defaults =
        required_assumption(embedded_registry_or_panic().constructor_defaults(profile_id));
    Ok(DealConfig {
        first_payment_date: closing_date.add_months(
            defaults
                .frequency
                .months()
                .expect("validated month-based frequency") as i32,
        )?,
        frequency: defaults.frequency,
        prepayment_spec: defaults.prepayment_spec,
        default_spec: defaults.default_spec,
        recovery_spec: defaults.recovery_spec,
        deal_metadata: Metadata::default(),
    })
}
