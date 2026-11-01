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
/// The optional payment calendar is applied before deal validation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StandardDealParams {
    /// Stable deal identifier used for pricing results and risk labels.
    pub id: InstrumentId,
    /// Current collateral balances and contractual asset terms, in the pool currency.
    pub pool: AssetPool,
    /// Capital structure, current note balances and coupon terms.
    pub tranches: TrancheStructure,
    /// Contractual deal inception; the first accrual boundary is one registry
    /// payment period later (capped at maturity).
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub closing_date: Date,
    /// Legal final date, strictly after closing.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub maturity: Date,
    /// Market-context discount curve used for tranche PV.
    pub discount_curve_id: CurveId,
    /// Optional holiday calendar for payment-date adjustment. Construction accepts
    /// `None`; simulation requires a calendar before generating payment dates.
    pub calendar_id: Option<CalendarId>,
}

/// Core instrument parameters shared across constructors
pub(super) struct InstrumentParams<'a> {
    pub pool: AssetPool,
    pub tranches: TrancheStructure,
    pub maturity: Date,
    pub discount_curve_id: &'a str,
}

/// The deal families [`StructuredCredit::new_standard`] accepts, each with the
/// registry profile that supplies its frequency and credit assumptions.
const STANDARD_FAMILIES: [(DealType, &str); 4] = [
    (DealType::Abs, "abs_auto_standard"),
    (DealType::Clo, "clo_standard"),
    (DealType::Cmbs, "cmbs_standard"),
    (DealType::Rmbs, "rmbs_standard"),
];

/// One deal family [`StructuredCredit::new_standard`] accepts; an entry of
/// [`standard_deal_families`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StandardDealFamily {
    /// The `deal_type` argument of [`StructuredCredit::new_standard`].
    pub deal_type: DealType,
    /// Registry profile that family's deal takes its assumptions from.
    pub profile_id: String,
    /// Complete [`StandardDealParams`] that `new_standard` accepts for this family,
    /// with its `id`, tranche ids and `pool.deal_type` naming the family. Its schema
    /// is `standard_deal_params`, not repeated here.
    #[cfg_attr(feature = "json-schema", schemars(with = "serde_json::Value"))]
    pub example: StandardDealParams,
}

/// List the deal families [`StructuredCredit::new_standard`] accepts, the one
/// place a host reads them from, with a coherent example of each family's inputs.
///
/// # Returns
///
/// One [`StandardDealFamily`] per family, in `abs`, `clo`, `cmbs`, `rmbs` order.
/// Each example is a USD 100m fixed-rate collateral row and one senior note, in
/// the family's schedule day count, closing 2024-01-01 and maturing 2034-01-01 on
/// the `USD-OIS` curve and the `nyse` calendar.
///
/// # Errors
///
/// Returns an error if an example's tranche cannot be constructed, which the
/// fixed inputs do not trigger.
pub fn standard_deal_families() -> finstack_quant_core::Result<Vec<StandardDealFamily>> {
    STANDARD_FAMILIES
        .into_iter()
        .map(|(deal_type, profile_id)| {
            Ok(StandardDealFamily {
                deal_type,
                profile_id: profile_id.to_string(),
                example: standard_family_example(deal_type)?,
            })
        })
        .collect()
}

/// The inputs a host starts from for `deal_type`; see [`standard_deal_families`].
fn standard_family_example(deal_type: DealType) -> finstack_quant_core::Result<StandardDealParams> {
    use finstack_quant_core::currency::Currency;
    let closing_date = time::macros::date!(2024 - 01 - 01);
    let maturity = time::macros::date!(2034 - 01 - 01);
    let family = deal_type.to_string().to_uppercase();
    let balance = Money::from((100_000_000_i64, Currency::USD));
    let mut pool = AssetPool::new(format!("{family}-POOL"), deal_type, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "COLLATERAL-1",
        balance,
        0.07,
        maturity,
        deal_type.schedule_day_count(),
    ));
    let tranche = Tranche::new(
        format!("{family}NOTES-A"),
        0.0,
        100.0,
        TrancheSeniority::Senior,
        balance,
        RateSpec::Fixed { rate: 0.06 },
        maturity,
    )?;
    Ok(StandardDealParams {
        id: format!("{family}-EXAMPLE").into(),
        pool,
        tranches: TrancheStructure::new(vec![tranche])?,
        closing_date,
        maturity,
        discount_curve_id: "USD-OIS".into(),
        calendar_id: Some("nyse".into()),
    })
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
        Ok(StructuredCredit::new_standard(
            crate::structured_credit::DealType::Clo,
            crate::structured_credit::StandardDealParams {
                id: "CLO-EXAMPLE".into(),
                pool,
                tranches,
                closing_date: closing,
                maturity: legal,
                discount_curve_id: "USD-OIS".into(),
                calendar_id: None,
            },
        )?
        .with_calendar_id("nyse"))
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
    /// Selects the registry profile, constructs the deal once, applies
    /// `params.calendar_id`, then runs [`Instrument::validate_for_pricing`].
    /// A missing calendar is accepted here; simulation requires one.
    ///
    /// # Arguments
    ///
    /// * `deal_type` - Deal family selecting the registry constructor profile;
    ///   one of [`DealType::Abs`], [`DealType::Clo`], [`DealType::Cmbs`] or
    ///   [`DealType::Rmbs`], as [`standard_deal_families`] lists them.
    /// * `params` - Deal identifier, collateral pool, tranches, closing and
    ///   maturity dates, discount curve and optional payment calendar.
    ///   `params.pool.deal_type` must be `deal_type`; the `abs` family also takes
    ///   auto and card pools, which have no constructor of their own.
    ///
    /// # Errors
    ///
    /// Returns a validation error if `deal_type` has no standard constructor
    /// (CBO, auto, card), if `params.pool.deal_type` contradicts it, if the first
    /// registry payment period exceeds the
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
        let Some((_, profile_id)) = STANDARD_FAMILIES
            .into_iter()
            .find(|(family, _)| *family == deal_type)
        else {
            return Err(finstack_quant_core::Error::Validation(format!(
                "deal type {deal_type:?} has no standard constructor; use the StructuredCredit builder"
            )));
        };
        // Auto and card pools are ABS, the family they are built under.
        let coherent = pool.deal_type == deal_type
            || (deal_type == DealType::Abs
                && matches!(pool.deal_type, DealType::Auto | DealType::Card));
        if !coherent {
            return Err(finstack_quant_core::Error::Validation(format!(
                "pool {} has deal type {}, which contradicts the {deal_type} family",
                pool.id, pool.deal_type
            )));
        }
        let mut deal = Self::new_with_deal_config(
            id,
            deal_type,
            InstrumentParams {
                pool,
                tranches,
                maturity,
                discount_curve_id: discount_curve_id.as_str(),
            },
            deal_config_from_registry(profile_id, closing_date)?,
            closing_date,
        );
        deal.calendar_id = calendar_id;
        deal.validate_for_pricing()?;
        Ok(deal)
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
