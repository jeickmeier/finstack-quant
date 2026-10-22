//! Builder API Extensions for Capital Structure
//!
//! This module provides fluent builder methods for adding capital structure
//! instruments to a financial model.

use crate::builder::ModelBuilder;
use crate::error::Result;
use crate::types::{CapitalStructureSpec, DebtInstrumentSpec, FinancialStatementInstrument};
use finstack_quant_cashflows::builder::FloatingLegCompounding;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId, Rate};
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::rates::irs::InterestRateSwap;
use finstack_quant_valuations::instruments::{FixedLegSpec, FloatLegSpec};
use rust_decimal::Decimal;

/// Inputs for appending a bond with a regional market convention.
#[derive(Debug)]
pub struct BondConventionParams {
    /// Capital-structure identifier; market-aware evaluation rejects duplicates.
    pub id: String,
    /// Principal in the tagged currency's major units.
    pub notional: Money,
    /// Typed annual decimal coupon rate, such as 0.03 for 3%.
    pub coupon_rate: Rate,
    /// Bond issue date, strictly before maturity.
    pub issue_date: Date,
    /// Final contractual bond maturity date.
    pub maturity_date: Date,
    /// Regional preset controlling coupon frequency, day count, and calendar.
    pub convention: finstack_quant_valuations::instruments::BondConvention,
    /// Discount-curve lookup identifier used during market-aware evaluation.
    pub discount_curve_id: String,
}

/// Financial terms for appending a pay-fixed, zero-spread interest-rate swap.
#[derive(Debug)]
pub struct SwapParams {
    /// Capital-structure identifier; market-aware evaluation rejects duplicates.
    pub id: String,
    /// Swap notional in the tagged currency's major units.
    pub notional: Money,
    /// Finite annual fixed-leg decimal rate, such as 0.04 for 4%.
    pub fixed_rate: f64,
    /// Effective accrual start date, strictly before maturity.
    pub start_date: Date,
    /// Final contractual swap maturity date.
    pub maturity_date: Date,
    /// Discount-curve lookup identifier used at evaluation.
    pub discount_curve_id: String,
    /// Floating forward-curve identifier, coherent with simple term-index compounding.
    pub forward_curve_id: String,
}

/// Schedule and accrual conventions for a simple-compounding swap.
#[derive(Debug)]
pub struct SwapConventions {
    /// Contractual fixed-leg payment cadence.
    pub fixed_frequency: Tenor,
    /// Fixed-leg accrual year-fraction convention.
    pub fixed_day_count: DayCount,
    /// Contractual floating-leg payment and fixing cadence.
    pub float_frequency: Tenor,
    /// Floating-leg accrual year-fraction convention.
    pub float_day_count: DayCount,
    /// Schedule-date rolling rule on the currency's settlement calendar.
    pub business_day_convention: BusinessDayConvention,
}

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
    /// Append a fixed-rate bond only after its construction succeeds.
    ///
    /// A rejected instrument leaves every accumulated builder field intact.
    /// Uses US corporate conventions: 30/360 with semi-annual coupons.
    ///
    /// # Arguments
    ///
    /// * `id` - Instrument identifier recorded in the capital structure; market-aware evaluation rejects duplicate identifiers.
    /// * `notional` - Principal in the tagged currency's major units.
    /// * `coupon_rate` - Annual coupon as a finite decimal fraction, such as 0.05 for 5%.
    /// * `issue_date` - Bond issue date, strictly before maturity.
    /// * `maturity_date` - Final contractual maturity date.
    /// * `discount_curve_id` - Discount-curve lookup identifier used during market-aware evaluation.
    ///
    /// # Errors
    ///
    /// Returns a validation or construction error for an invalid rate, schedule,
    /// notional, or identifier, without changing the builder.
    pub fn add_bond(
        &mut self,
        id: impl Into<String>,
        notional: Money,
        coupon_rate: f64,
        issue_date: Date,
        maturity_date: Date,
        discount_curve_id: impl Into<String>,
    ) -> Result<&mut Self> {
        let id_str: String = id.into();

        let bond = Bond::fixed(
            InstrumentId::new(&id_str),
            notional,
            Rate::from_decimal(coupon_rate)?,
            issue_date,
            maturity_date,
            finstack_quant_core::dates::StubKind::ShortFront,
            CurveId::new(discount_curve_id),
        )
        .map_err(|e| {
            crate::error::Error::build(format!("Failed to create bond '{}': {}", id_str, e))
        })?;

        push_debt(
            &mut self.capital_structure,
            id_str,
            FinancialStatementInstrument::Bond(bond),
        );
        Ok(self)
    }

    /// Append a bond with regional conventions without changing state on failure.
    ///
    /// # Arguments
    ///
    /// * `params` - Bond identity, tagged notional, annual decimal coupon,
    ///   contractual dates, regional convention, and discount-curve lookup identifier.
    ///
    /// # Errors
    ///
    /// Returns a bond construction error for an invalid schedule, notional, or
    /// identifier; the accumulated model remains unchanged.
    pub fn add_bond_with_convention(&mut self, params: BondConventionParams) -> Result<&mut Self> {
        let BondConventionParams {
            id: id_str,
            notional,
            coupon_rate,
            issue_date,
            maturity_date,
            convention,
            discount_curve_id,
        } = params;

        let bond = Bond::with_convention(
            InstrumentId::new(&id_str),
            notional,
            coupon_rate,
            issue_date,
            maturity_date,
            convention,
            CurveId::new(discount_curve_id),
        )
        .map_err(|e| {
            crate::error::Error::build(format!("Failed to create bond '{}': {}", id_str, e))
        })?;

        push_debt(
            &mut self.capital_structure,
            id_str,
            FinancialStatementInstrument::Bond(bond),
        );
        Ok(self)
    }

    /// Append a US-convention swap only after its construction succeeds.
    ///
    /// Uses pay-fixed, zero-spread, simple compounding with semi-annual 30/360
    /// fixed coupons and quarterly ACT/360 floating coupons. Rejected inputs
    /// leave the builder intact.
    ///
    /// # Arguments
    ///
    /// * `params` - Swap identity, tagged notional, annual fixed decimal rate,
    ///   contractual dates, and discount/forward curve lookup identifiers.
    ///
    /// # Errors
    ///
    /// Returns a validation or construction error for an invalid rate, schedule,
    /// index convention, or identifier without changing the builder.
    pub fn add_swap(&mut self, params: SwapParams) -> Result<&mut Self> {
        self.add_swap_with_conventions(
            params,
            SwapConventions {
                fixed_frequency: Tenor::semi_annual(),
                fixed_day_count: DayCount::Thirty360,
                float_frequency: Tenor::quarterly(),
                float_day_count: DayCount::Act360,
                business_day_convention: BusinessDayConvention::ModifiedFollowing,
            },
        )
    }

    /// Append a custom-convention swap without changing state on failure.
    ///
    /// Builds a pay-fixed, zero-spread swap with simple floating compounding
    /// and the caller's payment and day-count conventions.
    ///
    /// # Arguments
    ///
    /// * `params` - Swap identity, tagged notional, annual fixed decimal rate,
    ///   contractual dates, and discount/forward curve lookup identifiers.
    /// * `conventions` - Fixed/floating payment frequencies and day counts plus
    ///   the business-day rolling rule; floating compounding remains simple.
    ///
    /// # Errors
    ///
    /// Returns a validation or construction error for an invalid rate,
    /// schedule, index convention, or identifier without changing the builder.
    pub fn add_swap_with_conventions(
        &mut self,
        params: SwapParams,
        conventions: SwapConventions,
    ) -> Result<&mut Self> {
        let SwapParams {
            id: id_str,
            notional,
            fixed_rate,
            start_date,
            maturity_date,
            discount_curve_id,
            forward_curve_id,
        } = params;
        let SwapConventions {
            fixed_frequency,
            fixed_day_count,
            float_frequency,
            float_day_count,
            business_day_convention,
        } = conventions;
        use finstack_quant_valuations::instruments::PayReceive;

        let rate_decimal = finstack_quant_core::decimal::f64_to_decimal(fixed_rate)?;

        // The swap rolls on its currency's standard rates calendar. A currency
        // without one is rejected rather than silently rolled on US holidays.
        let currency = notional.currency();
        let calendar_id =
            finstack_quant_valuations::instruments::pricing::default_rate_calendar_id(currency)
                .ok_or_else(|| {
                    crate::error::Error::build(format!(
                        "swap '{id_str}': no standard rates calendar is defined for {currency}; \
                         add the swap with `add_debt` and explicit leg calendars"
                    ))
                })?
                .to_string();

        let discount_curve_id = CurveId::new(discount_curve_id);
        let forward_curve_id = CurveId::new(forward_curve_id);

        let fixed = FixedLegSpec {
            discount_curve_id: discount_curve_id.clone(),
            rate: rate_decimal,
            frequency: fixed_frequency,
            day_count: fixed_day_count,
            business_day_convention,
            calendar_id: Some(calendar_id.clone().into()),
            stub: StubKind::None,
            start: start_date,
            end: maturity_date,

            payment_lag_days: 0,
            end_of_month: false,
        };

        let float = FloatLegSpec {
            discount_curve_id,
            forward_curve_id,
            spread_bp: Decimal::ZERO,
            frequency: float_frequency,
            day_count: float_day_count,
            business_day_convention,
            calendar_id: Some(calendar_id.into()),
            stub: StubKind::None,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            start: start_date,
            end: maturity_date,
            compounding: FloatingLegCompounding::Simple,
            payment_lag_days: 0,
            end_of_month: false,
        };

        let swap = InterestRateSwap::builder()
            .id(InstrumentId::new(&id_str))
            .notional(notional)
            .side(PayReceive::Pay)
            .fixed_leg(fixed)
            .float_leg(float)
            .build()?;
        push_debt(
            &mut self.capital_structure,
            id_str,
            FinancialStatementInstrument::InterestRateSwap(swap),
        );
        Ok(self)
    }

    /// Add a typed debt instrument.
    ///
    /// This is the path for any instrument type not covered by the convenience
    /// methods (`add_bond`, `add_swap`).
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
    /// When this is not set, reporting totals use
    /// [`finstack_quant_core::money::fx::FxConversionPolicy::PeriodEnd`]:
    /// already-aggregated `cs.*` cash items and balances convert on the inclusive
    /// period-end date. Pass
    /// [`finstack_quant_core::money::fx::FxConversionPolicy::CashflowDate`] only
    /// when the model must convert on each contractual flow date instead.
    ///
    /// # Arguments
    ///
    /// * `policy` - Rate-selection hint applied to each reporting-currency
    ///   conversion of a period-aggregated `cs.*` bucket. `PeriodEnd` uses the
    ///   inclusive period-end snapshot; `CashflowDate` uses the flow date;
    ///   `PeriodAverage` and `Custom` are forwarded to the FX provider.
    pub fn fx_policy(
        &mut self,
        policy: finstack_quant_core::money::fx::FxConversionPolicy,
    ) -> &mut Self {
        ensure_capital_structure(&mut self.capital_structure).fx_policy = Some(policy);
        self
    }

    /// Configure waterfall specification for dynamic cash flow allocation.
    ///
    /// # Arguments
    ///
    /// * `waterfall_spec` - Priority-of-payments, pre-waterfall cash node,
    ///   optional ECF sweep / PIK toggle, payment classes, and optional
    ///   mandatory / voluntary prepay nodes.
    ///
    /// # Example
    /// ```
    /// use finstack_quant_statements::capital_structure::{WaterfallSpec, EcfSweepSpec};
    /// use finstack_quant_statements::builder::ModelBuilder;
    ///
    /// let waterfall = WaterfallSpec {
    ///     ecf_sweep: Some(EcfSweepSpec {
    ///         ebitda_node: "ebitda".to_string(),
    ///         taxes_node: Some("taxes".to_string()),
    ///         capex_node: Some("capex".to_string()),
    ///         working_capital_node: Some("wc_change".to_string()),
    ///         cash_interest_node: None,
    ///         sweep_percentage: 0.5,  // 50% sweep
    ///         target_instrument_id: Some("TL-A".to_string()),
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
