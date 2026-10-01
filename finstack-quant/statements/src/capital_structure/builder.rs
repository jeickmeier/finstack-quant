//! Builder API Extensions for Capital Structure
//!
//! This module provides fluent builder methods for adding capital structure
//! instruments to a financial model.

use crate::builder::ModelBuilder;
use crate::error::Result;
use crate::types::{CapitalStructureSpec, DebtInstrumentSpec, FinancialStatementInstrument};
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId, Rate};
use finstack_quant_valuations::instruments::rates::irs::FloatingLegCompounding;
use finstack_quant_valuations::instruments::{Bond, FixedLegSpec, FloatLegSpec, InterestRateSwap};
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

/// Add a typed bond payload to the capital structure.
fn push_bond(
    cs: &mut Option<CapitalStructureSpec>,
    id_str: String,
    bond: Bond,
) -> crate::error::Result<()> {
    let spec = FinancialStatementInstrument::Bond(bond);
    ensure_capital_structure(cs)
        .debt_instruments
        .push(DebtInstrumentSpec { id: id_str, spec });
    Ok(())
}

/// Add a typed swap payload to the capital structure.
fn push_swap(
    cs: &mut Option<CapitalStructureSpec>,
    id_str: String,
    swap: finstack_quant_valuations::instruments::InterestRateSwap,
) -> crate::error::Result<()> {
    let spec = FinancialStatementInstrument::InterestRateSwap(swap);
    ensure_capital_structure(cs)
        .debt_instruments
        .push(DebtInstrumentSpec { id: id_str, spec });
    Ok(())
}

/// Default settlement-calendar id for a currency.
///
/// Maps the major currencies to their standard market calendars so a swap's
/// coupon dates roll on the right holiday set (e.g. EUR → TARGET2, not NYSE).
/// Unmapped currencies fall back to the US calendar.
fn default_calendar_for(currency: finstack_quant_core::currency::Currency) -> &'static str {
    use finstack_quant_core::currency::Currency;
    match currency {
        Currency::EUR => "target2",
        Currency::GBP => "gblo",
        Currency::JPY => "jpto",
        _ => "usny",
    }
}

/// Build an `InterestRateSwap` from leg parameters.
#[allow(clippy::too_many_arguments)]
fn build_swap_internal(
    id_str: &str,
    notional: Money,
    fixed_rate: f64,
    start_date: Date,
    maturity_date: Date,
    discount_curve_id: String,
    forward_curve_id: String,
    fixed_frequency: Tenor,
    fixed_day_count: DayCount,
    float_frequency: Tenor,
    float_day_count: DayCount,
    business_day_convention: BusinessDayConvention,
) -> crate::error::Result<InterestRateSwap> {
    use finstack_quant_valuations::instruments::PayReceive;

    let rate_decimal = Decimal::try_from(fixed_rate).map_err(|_| {
        crate::error::Error::InvalidInput(format!(
            "Invalid fixed rate: {} cannot be converted to Decimal.",
            fixed_rate
        ))
    })?;

    // Default the settlement calendar from the swap's currency rather than
    // hardcoding the US calendar for every leg — a EUR swap must roll on
    // TARGET2, not NYSE holidays.
    let calendar_id = default_calendar_for(notional.currency()).to_string();

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
        par_method: None,
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

    Ok(InterestRateSwap::builder()
        .id(InstrumentId::new(id_str))
        .notional(notional)
        .side(PayReceive::Pay)
        .fixed_leg(fixed)
        .float_leg(float)
        .build()?)
}

impl<State> ModelBuilder<State> {
    /// Add a bond instrument to the capital structure specification.
    ///
    /// Uses `Bond::fixed()` with default conventions (semi-annual, 30/360).
    /// For non-standard conventions (e.g., EUR bonds with ACT/ACT), use
    /// [`add_debt`](Self::add_debt) with a pre-built `Bond` payload.
    ///
    /// # Arguments
    /// * `id` - Unique instrument identifier
    /// * `notional` - Principal in the tagged currency's major units.
    /// * `coupon_rate` - Annual decimal coupon rate, such as 0.05 for 5%.
    /// * `issue_date` - Contractual issue and coupon accrual start date; must precede maturity.
    /// * `maturity_date` - Final contractual repayment date; must follow the issue date.
    /// * `discount_curve_id` - Discount curve ID for pricing
    ///
    /// # Returns
    /// Updated builder with the bond appended to the capital-structure spec.
    /// The resulting specification stores the tagged valuations JSON needed by
    /// [`Evaluator::evaluate_with_market`](crate::evaluator::Evaluator::evaluate_with_market).
    /// It does not price the bond or check that `id` is unique in the complete
    /// capital structure; market-aware evaluation rejects duplicate identifiers.
    ///
    /// # Example
    /// ```
    /// use finstack_quant_statements::builder::ModelBuilder;
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// use time::macros::date;
    ///
    /// # fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    /// let issue_date = date!(2025-01-15);
    /// let maturity_date = date!(2030-01-15);
    ///
    /// let builder = ModelBuilder::new("cs-model")
    ///     .add_bond(
    ///         "BOND-001",
    ///         Money::from((10_000_000_i64, Currency::USD)),
    ///         0.05, // 5% coupon
    ///         issue_date,
    ///         maturity_date,
    ///         "USD-OIS",
    ///     )?;
    /// # let _ = builder;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a build error if the fixed-bond constructor rejects the dates,
    /// rate, notional, or curve configuration, or if the tagged instrument
    /// cannot be serialized into the model specification.
    pub fn add_bond(
        mut self,
        id: impl Into<String>,
        notional: Money,
        coupon_rate: f64,
        issue_date: Date,
        maturity_date: Date,
        discount_curve_id: impl Into<String>,
    ) -> Result<Self> {
        self.try_add_bond(
            id,
            notional,
            coupon_rate,
            issue_date,
            maturity_date,
            discount_curve_id,
        )?;
        Ok(self)
    }

    /// Append a fixed-rate bond only after its construction succeeds.
    ///
    /// A rejected instrument leaves every accumulated builder field intact.
    /// Uses the same US corporate conventions as [`add_bond`](Self::add_bond).
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
    pub fn try_add_bond(
        &mut self,
        id: impl Into<String>,
        notional: Money,
        coupon_rate: f64,
        issue_date: Date,
        maturity_date: Date,
        discount_curve_id: impl Into<String>,
    ) -> Result<()> {
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

        push_bond(&mut self.capital_structure, id_str, bond)
    }

    /// Add a bond instrument with a market convention preset.
    ///
    /// This overload applies regional day count, coupon frequency, and calendar
    /// conventions automatically. The default `add_bond` uses US corporate bond
    /// conventions (30/360, semi-annual, T+1).
    ///
    /// # Arguments
    /// * `id` - Unique instrument identifier
    /// * `notional` - Principal in the tagged currency's major units.
    /// * `coupon_rate` - Typed annual decimal coupon rate, such as 0.03 for 3%.
    /// * `issue_date` - Contractual issue and coupon accrual start date; must precede maturity.
    /// * `maturity_date` - Final contractual repayment date; must follow the issue date.
    /// * `convention` - Regional coupon, day-count, and calendar preset, such as `BondConvention::GermanBund`.
    /// * `discount_curve_id` - Discount curve ID for pricing
    ///
    /// The convention controls the coupon schedule and date conventions; its
    /// choice must match the bond's legal terms and the supplied discount curve
    /// must be available at evaluation time. This method records a tagged
    /// instrument specification but does not price it or enforce global
    /// uniqueness of `id`.
    ///
    /// # Example
    /// ```
    /// use finstack_quant_statements::builder::ModelBuilder;
    /// use finstack_quant_valuations::instruments::BondConvention;
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::types::Rate;
    /// use time::macros::date;
    ///
    /// # fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    /// let builder = ModelBuilder::new("cs-model")
    ///     .add_bond_with_convention(
    ///         "BUND-001",
    ///         Money::from((10_000_000_i64, Currency::EUR)),
    ///         Rate::from_decimal(0.03).expect("valid rate fixture"),
    ///         date!(2025-01-15),
    ///         date!(2030-01-15),
    ///         BondConvention::GermanBund,
    ///         "EUR-OIS",
    ///     )?;
    /// # let _ = builder;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a build error if the convention-aware bond constructor rejects
    /// the rate, dates, notional, or curve configuration, or if the resulting
    /// tagged instrument cannot be serialized.
    #[allow(clippy::too_many_arguments)]
    pub fn add_bond_with_convention(
        mut self,
        id: impl Into<String>,
        notional: Money,
        coupon_rate: finstack_quant_core::types::Rate,
        issue_date: Date,
        maturity_date: Date,
        convention: finstack_quant_valuations::instruments::BondConvention,
        discount_curve_id: impl Into<String>,
    ) -> Result<Self> {
        self.try_add_bond_with_convention(BondConventionParams {
            id: id.into(),
            notional,
            coupon_rate,
            issue_date,
            maturity_date,
            convention,
            discount_curve_id: discount_curve_id.into(),
        })?;
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
    pub fn try_add_bond_with_convention(&mut self, params: BondConventionParams) -> Result<()> {
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

        push_bond(&mut self.capital_structure, id_str, bond)
    }

    /// Add an interest rate swap to the capital structure.
    ///
    /// Uses US market conventions: fixed leg semi-annual 30/360, float leg quarterly ACT/360,
    /// both Modified Following. For non-USD or non-standard conventions, use
    /// [`add_swap_with_conventions`](Self::add_swap_with_conventions).
    ///
    /// # Arguments
    ///
    /// * `id` - Unique instrument identifier stored on the capital-structure debt entry
    /// * `notional` - Swap notional as [`Money`] in the trade currency's major units
    /// * `fixed_rate` - Fixed leg rate as a decimal (for example 0.04 for 4%)
    /// * `start_date` - Contractual accrual start date for both legs; must precede maturity.
    /// * `maturity_date` - Final contractual accrual end date for both legs; must follow the start date.
    /// * `discount_curve_id` - Market-context ID of the discount curve used at evaluation
    /// * `forward_curve_id` - Market-context ID of the floating-leg forward curve
    ///
    /// # Example
    /// ```
    /// use finstack_quant_statements::builder::ModelBuilder;
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// use time::macros::date;
    ///
    /// # fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    /// let builder = ModelBuilder::new("cs-model")
    ///     .add_swap(
    ///         "SWAP-001",
    ///         Money::from((5_000_000_i64, Currency::USD)),
    ///         0.04,
    ///         date!(2025-01-15),
    ///         date!(2030-01-15),
    ///         "USD-OIS",
    ///         "USD-SOFR-3M",
    ///     )?;
    /// # let _ = builder;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// The swap is created with the `Pay` side, zero floating spread, simple
    /// compounding, and a settlement calendar chosen from the notional
    /// currency (EUR → TARGET2, GBP → GBLO, JPY → JPTO, otherwise USNY). It
    /// is serialized into the capital-structure specification; pricing occurs
    /// only during market-aware evaluation.
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error if `fixed_rate` cannot be represented as
    /// a finite decimal, a build error if the swap schedule or identifiers are
    /// invalid, or a serialization error if the tagged instrument cannot be
    /// stored. It does not check that the curve identifiers exist in market
    /// data; that happens at evaluation time.
    #[allow(clippy::too_many_arguments)]
    pub fn add_swap(
        mut self,
        id: impl Into<String>,
        notional: Money,
        fixed_rate: f64,
        start_date: Date,
        maturity_date: Date,
        discount_curve_id: impl Into<String>,
        forward_curve_id: impl Into<String>,
    ) -> Result<Self> {
        self.try_add_swap(SwapParams {
            id: id.into(),
            notional,
            fixed_rate,
            start_date,
            maturity_date,
            discount_curve_id: discount_curve_id.into(),
            forward_curve_id: forward_curve_id.into(),
        })?;
        Ok(self)
    }

    /// Append a US-convention swap only after its construction succeeds.
    ///
    /// Uses the pay-fixed, zero-spread, simple-compounding conventions of
    /// [`add_swap`](Self::add_swap). Rejected inputs leave the builder intact.
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
    pub fn try_add_swap(&mut self, params: SwapParams) -> Result<()> {
        self.try_add_swap_with_conventions(
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

    /// Add an interest rate swap with custom conventions.
    ///
    /// This overload exposes day count, frequency, and business day convention
    /// parameters for non-USD swaps (e.g., EUR swaps with ACT/360 annual fixed,
    /// GBP swaps with ACT/365F semi-annual fixed).
    ///
    /// The default [`add_swap`](Self::add_swap) uses US conventions:
    /// - Fixed: Semi-annual, 30/360, Modified Following
    /// - Float: Quarterly, ACT/360, Modified Following
    ///
    /// The created swap is on the `Pay` side with zero floating spread and
    /// simple floating compounding. The caller is responsible for choosing
    /// coherent frequencies, day-count conventions, and curve identifiers for
    /// the market and legal confirmation being modeled.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique instrument identifier stored on the capital-structure debt entry
    /// * `notional` - Swap notional as [`Money`] in the trade currency's major units
    /// * `fixed_rate` - Fixed leg rate as a decimal (for example 0.04 for 4%)
    /// * `start_date` - Contractual accrual start date for both legs; must precede maturity.
    /// * `maturity_date` - Final contractual accrual end date for both legs; must follow the start date.
    /// * `discount_curve_id` - Market-context ID of the discount curve used at evaluation
    /// * `forward_curve_id` - Market-context ID of the floating-leg forward curve
    /// * `fixed_frequency` - Payment frequency of the fixed leg
    /// * `fixed_day_count` - Day-count convention applied to the fixed leg
    /// * `float_frequency` - Payment / fixing frequency of the floating leg
    /// * `float_day_count` - Day-count convention applied to the floating leg
    /// * `business_day_convention` - Business-day convention used when adjusting schedule dates
    ///
    /// # Errors
    ///
    /// Returns an invalid-input error if `fixed_rate` cannot be represented as
    /// a finite decimal, a build error if the swap configuration is invalid,
    /// or a serialization error if the tagged instrument cannot be stored.
    /// Curve availability and pricing consistency are validated later by
    /// market-aware evaluation.
    #[allow(clippy::too_many_arguments)]
    pub fn add_swap_with_conventions(
        mut self,
        id: impl Into<String>,
        notional: Money,
        fixed_rate: f64,
        start_date: Date,
        maturity_date: Date,
        discount_curve_id: impl Into<String>,
        forward_curve_id: impl Into<String>,
        fixed_frequency: Tenor,
        fixed_day_count: DayCount,
        float_frequency: Tenor,
        float_day_count: DayCount,
        business_day_convention: BusinessDayConvention,
    ) -> Result<Self> {
        self.try_add_swap_with_conventions(
            SwapParams {
                id: id.into(),
                notional,
                fixed_rate,
                start_date,
                maturity_date,
                discount_curve_id: discount_curve_id.into(),
                forward_curve_id: forward_curve_id.into(),
            },
            SwapConventions {
                fixed_frequency,
                fixed_day_count,
                float_frequency,
                float_day_count,
                business_day_convention,
            },
        )?;
        Ok(self)
    }

    /// Append a custom-convention swap without changing state on failure.
    ///
    /// Builds the same pay-fixed, simple-compounding swap as
    /// [`add_swap_with_conventions`](Self::add_swap_with_conventions).
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
    pub fn try_add_swap_with_conventions(
        &mut self,
        params: SwapParams,
        conventions: SwapConventions,
    ) -> Result<()> {
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
        let swap = build_swap_internal(
            &id_str,
            notional,
            fixed_rate,
            start_date,
            maturity_date,
            discount_curve_id,
            forward_curve_id,
            fixed_frequency,
            fixed_day_count,
            float_frequency,
            float_day_count,
            business_day_convention,
        )?;
        push_swap(&mut self.capital_structure, id_str, swap)
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
    /// let builder = ModelBuilder::new("cs-model").add_debt("RCF-A", instrument());
    /// # let _ = builder;
    /// ```
    ///
    /// # Arguments
    ///
    /// * `id` - Unique instrument identifier stored on the capital-structure debt entry
    /// * `spec` - Typed bond, loan, revolver, convertible, swap, cap/floor, or swaption payload
    pub fn add_debt(mut self, id: impl Into<String>, spec: FinancialStatementInstrument) -> Self {
        ensure_capital_structure(&mut self.capital_structure)
            .debt_instruments
            .push(DebtInstrumentSpec {
                id: id.into(),
                spec,
            });

        self
    }

    /// Set an explicit reporting currency for capital-structure totals.
    pub fn reporting_currency(mut self, currency: finstack_quant_core::currency::Currency) -> Self {
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
    pub fn fx_policy(mut self, policy: finstack_quant_core::money::fx::FxConversionPolicy) -> Self {
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
    /// let builder = ModelBuilder::new("cs-model").waterfall(waterfall);
    /// # let _ = builder;
    /// ```
    pub fn waterfall(mut self, waterfall_spec: crate::capital_structure::WaterfallSpec) -> Self {
        ensure_capital_structure(&mut self.capital_structure).waterfall = Some(waterfall_spec);
        self
    }
}
