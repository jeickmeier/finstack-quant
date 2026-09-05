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
        calendar_id: Some(calendar_id.clone()),
        stub: StubKind::None,
        start: start_date,
        end: maturity_date,
        par_method: None,
        compounding_simple: true,
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
        calendar_id: Some(calendar_id),
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
        .fixed(fixed)
        .float(float)
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
    /// * `notional` - Principal amount
    /// * `coupon_rate` - Annual coupon rate (e.g., 0.05 for 5%)
    /// * `issue_date` - Bond issue date
    /// * `maturity_date` - Bond maturity date
    /// * `discount_curve_id` - Discount curve ID for pricing
    ///
    /// # Returns
    /// Updated builder with the bond appended to the capital-structure spec.
    /// The resulting specification stores the tagged valuations JSON needed by
    /// [`Evaluator::evaluate_with_market`](crate::evaluator::Evaluator::evaluate_with_market).
    /// It does not price the bond or check that `id` is unique in the complete
    /// capital structure; model-level validation remains the final boundary.
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

        push_bond(&mut self.capital_structure, id_str, bond)?;
        Ok(self)
    }

    /// Add a bond instrument with a market convention preset.
    ///
    /// This overload applies regional day count, coupon frequency, and calendar
    /// conventions automatically. The default `add_bond` uses US corporate bond
    /// conventions (30/360, semi-annual, T+1).
    ///
    /// # Arguments
    /// * `id` - Unique instrument identifier
    /// * `notional` - Principal amount
    /// * `coupon_rate` - Annual coupon rate as typed `Rate`
    /// * `issue_date` - Bond issue date
    /// * `maturity_date` - Bond maturity date
    /// * `convention` - Regional convention preset (e.g., `BondConvention::EurGovernment`)
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
        let id_str: String = id.into();

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

        push_bond(&mut self.capital_structure, id_str, bond)?;
        Ok(self)
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
    /// * `start_date` - Effective / accrual start date of the swap schedule
    /// * `maturity_date` - Final maturity date of the swap schedule
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
        let id_str: String = id.into();
        let swap = build_swap_internal(
            &id_str,
            notional,
            fixed_rate,
            start_date,
            maturity_date,
            discount_curve_id.into(),
            forward_curve_id.into(),
            Tenor::semi_annual(),
            DayCount::Thirty360,
            Tenor::quarterly(),
            DayCount::Act360,
            BusinessDayConvention::ModifiedFollowing,
        )?;
        push_swap(&mut self.capital_structure, id_str, swap)?;
        Ok(self)
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
    /// * `start_date` - Effective / accrual start date of the swap schedule
    /// * `maturity_date` - Final maturity date of the swap schedule
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
        let id_str: String = id.into();
        let swap = build_swap_internal(
            &id_str,
            notional,
            fixed_rate,
            start_date,
            maturity_date,
            discount_curve_id.into(),
            forward_curve_id.into(),
            fixed_frequency,
            fixed_day_count,
            float_frequency,
            float_day_count,
            business_day_convention,
        )?;
        push_swap(&mut self.capital_structure, id_str, swap)?;
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
