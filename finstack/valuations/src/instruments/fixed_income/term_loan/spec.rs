//! Serde-stable specification types for term loans and DDTL features.
//!
//! This module defines the serializable specification structures for term loans
//! including delayed-draw term loans (DDTL), covenant events, amortization schedules,
//! and call provisions.
//!
//! # Overview
//!
//! All types in this module are designed for stable serialization with:
//! - `#[serde(deny_unknown_fields)]` to catch configuration errors
//! - Explicit field naming for long-lived pipelines
//! - Conversion to/from runtime [`TermLoan`](super::types::TermLoan) instances
//!
//! # Key Types
//!
//! - [`TermLoanSpec`]: Complete loan specification (serializable)
//! - [`DdtlSpec`]: Delayed-draw term loan features
//! - [`TermLoanCovenantEvents`]: Covenant-driven events
//! - [`AmortizationSpec`]: Principal repayment schedules
//! - [`LoanCallSchedule`]: Borrower prepayment options
//! - [`OidPolicy`]: Original issue discount handling
//! - [`OidEirSpec`]: Effective interest rate amortization settings
//!
//! # Quick Example
//!
//! ```text
//! use finstack_valuations::instruments::fixed_income::term_loan::spec::*;
//! use finstack_valuations::instruments::fixed_income::term_loan::RateSpec;
//! use finstack_cashflows::builder::specs::CouponType;
//! use finstack_valuations::instruments::pricing_overrides::PricingOverrides;
//! use finstack_core::money::Money;
//! use finstack_core::currency::Currency;
//! use finstack_core::dates::*;
//! use finstack_core::types::{InstrumentId, CurveId};
//! use time::Month;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let spec = TermLoanSpec {
//!     id: InstrumentId::new("TL-001"),
//!     discount_curve_id: CurveId::new("USD-CREDIT"),
//!     currency: Currency::USD,
//!     notional_limit: Some(Money::new(10_000_000.0, Currency::USD)),
//!     issue: create_date(2025, Month::January, 1)?,
//!     maturity: create_date(2030, Month::January, 1)?,
//!     rate: RateSpec::Fixed { rate_bp: 600 },
//!     frequency: Tenor::quarterly(),
//!     day_count: DayCount::Act360,
//!     bdc: BusinessDayConvention::ModifiedFollowing,
//!     calendar_id: None,
//!     stub: StubKind::None,
//!     amortization: AmortizationSpec::None,
//!     coupon_type: CouponType::Cash,
//!     upfront_fee: None,
//!     ddtl: None,
//!     covenants: None,
//!     credit_curve_id: None,
//!     pricing_overrides: PricingOverrides::default(),
//!     oid_eir: None,
//!     call_schedule: None,
//!     settlement_days: 2,
//! };
//! # Ok(())
//! # }
//! ```
//!
//! # See Also
//!
//! - [`super::types::TermLoan`] for the runtime instrument type
//! - `super::cashflows` for cashflow generation (internal module)
//! - term loan pricing module for valuation

use crate::instruments::pricing_overrides::PricingOverrides;
use finstack_core::currency::Currency;
use finstack_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_core::money::Money;
use finstack_core::types::{Bps, CurveId, InstrumentId};

use super::types::RateSpec;

/// Original Issue Discount (OID) policy for term loan origination.
///
/// OID represents the discount from par value at loan origination. The policy
/// determines how the discount is handled: withheld from proceeds or tracked separately.
///
/// # Industry Practice
///
/// OID is common in institutional term loans and private credit, particularly for:
/// - Leveraged buyout financing (LBO loans)
/// - Distressed refinancings
/// - High-yield institutional term loans
///
/// Typical OID ranges from 1-5% (100-500 bps) of par value.
///
/// # Accounting Treatment
///
/// OID affects accounting under GAAP/IFRS:
/// - **Withheld**: Reduces initial cash proceeds, increases effective yield
/// - **Separate**: May be accounted as upfront fee or amortized discount
///
/// For effective interest rate (EIR) amortization schedules, see [`OidEirSpec`].
///
/// # Variants
///
/// - `WithheldPct`: Discount as percentage withheld from funded amount
/// - `WithheldAmount`: Fixed amount withheld from funded proceeds
/// - `SeparatePct`: Percentage tracked separately, not withheld
/// - `SeparateAmount`: Fixed amount tracked separately
///
/// # Examples
///
/// ```text
/// use finstack_valuations::instruments::fixed_income::term_loan::spec::OidPolicy;
/// use finstack_core::money::Money;
/// use finstack_core::currency::Currency;
///
/// // 2% OID withheld from proceeds
/// let oid = OidPolicy::WithheldPct(200);  // 200 bps = 2%
///
/// // $50,000 fixed OID
/// let oid_fixed = OidPolicy::WithheldAmount(Money::new(50_000.0, Currency::USD));
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub enum OidPolicy {
    /// Discount as percentage (basis points) withheld from funded proceeds
    WithheldPct(i32),
    /// Fixed discount amount withheld from funded proceeds
    WithheldAmount(Money),
    /// Discount as percentage tracked separately for amortization
    SeparatePct(i32),
    /// Fixed discount amount tracked separately for amortization
    SeparateAmount(Money),
}

impl OidPolicy {
    /// Create a withheld OID percentage using typed basis points.
    pub fn withheld_pct_bps(bps: Bps) -> Self {
        Self::WithheldPct(bps.as_bps())
    }

    /// Create a separate OID percentage using typed basis points.
    pub fn separate_pct_bps(bps: Bps) -> Self {
        Self::SeparatePct(bps.as_bps())
    }
}

/// Optional configuration for effective interest rate (EIR) amortization schedules.
///
/// When enabled, EIR amortization schedules are computed for reporting using
/// the loan's full cashflow schedule (including OID effects).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct OidEirSpec {
    /// Include fee cashflows (upfront, commitment, usage) in the EIR schedule.
    ///
    /// Defaults to true because these fees are typically part of the effective yield.
    pub include_fees: bool,
}

impl Default for OidEirSpec {
    fn default() -> Self {
        Self { include_fees: true }
    }
}

/// Draw event for delayed-draw term loans (DDTL).
///
/// Represents a scheduled or actual draw against the commitment, reducing
/// available capacity and increasing outstanding principal.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DrawEvent {
    /// Date of the draw
    #[schemars(with = "String")]
    pub date: Date,
    /// Amount drawn from available commitment
    pub amount: Money,
}

/// Commitment step-down event for DDTL facilities.
///
/// Reduces the total commitment limit at a specified date, typically used
/// to match construction completion or covenant requirements.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommitmentStepDown {
    /// Effective date of the step-down
    #[schemars(with = "String")]
    pub date: Date,
    /// New (lower) commitment limit after step-down
    pub new_limit: Money,
}

/// Basis for calculating commitment fees on undrawn portions.
///
/// Determines the denominator for commitment fee calculations on
/// revolving or delayed-draw facilities.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub enum CommitmentFeeBase {
    /// Fee based on total undrawn amount only
    Undrawn,
    /// Fee based on commitment limit minus outstanding principal
    CommitmentMinusOutstanding,
}

/// Delayed-draw term loan (DDTL) specification.
///
/// Models a term loan with commitment period during which borrower may draw
/// down funds, subject to availability dates, step-downs, and fees.
///
/// # Industry Practice
///
/// DDTLs are common in:
/// - **Construction financing**: Funds released as construction milestones are met
/// - **Acquisition financing**: Delayed funding for earn-outs or contingent payments
/// - **Working capital facilities**: Drawn as needed within commitment period
///
/// Typical features:
/// - Commitment period: 6-24 months
/// - Commitment fees: 25-50 bps on undrawn amounts
/// - Usage fees: 0-25 bps on drawn amounts
/// - Step-downs: Commitment reduces at milestones (e.g., construction completion)
///
/// # Fee Conventions
///
/// - **Commitment fee**: Paid on undrawn commitment (compensates lender for availability)
/// - **Usage fee**: Paid on drawn amounts (additive to interest margin)
/// - **OID**: May be withheld at each draw or tracked separately
///
/// # Examples
///
/// ```text
/// use finstack_valuations::instruments::fixed_income::term_loan::spec::*;
/// use finstack_core::money::Money;
/// use finstack_core::currency::Currency;
/// use finstack_core::dates::create_date;
/// use time::Month;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let ddtl = DdtlSpec {
///     commitment_limit: Money::new(10_000_000.0, Currency::USD),
///     availability_start: create_date(2025, Month::January, 1)?,
///     availability_end: create_date(2026, Month::January, 1)?,
///     draws: vec![],
///     commitment_step_downs: vec![],
///     usage_fee_bp: 50,        // 50 bps usage fee
///     commitment_fee_bp: 25,   // 25 bps commitment fee
///     fee_base: CommitmentFeeBase::Undrawn,
///     oid_policy: None,
/// };
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DdtlSpec {
    /// Total commitment limit available for draws
    pub commitment_limit: Money,
    /// First date draws are permitted
    #[schemars(with = "String")]
    pub availability_start: Date,
    /// Last date draws are permitted (commitment expiry)
    #[schemars(with = "String")]
    pub availability_end: Date,
    /// Scheduled or actual draw events
    pub draws: Vec<DrawEvent>,
    /// Commitment step-down schedule
    pub commitment_step_downs: Vec<CommitmentStepDown>,
    /// Usage fee in basis points (on drawn amounts)
    pub usage_fee_bp: i32,
    /// Commitment fee in basis points (on undrawn amounts)
    pub commitment_fee_bp: i32,
    /// Basis for commitment fee calculation
    pub fee_base: CommitmentFeeBase,
    /// Original issue discount policy, if applicable
    pub oid_policy: Option<OidPolicy>,
}

impl DdtlSpec {
    /// Set usage fee using typed basis points.
    pub fn with_usage_fee_bps(mut self, usage_fee_bp: Bps) -> Self {
        self.usage_fee_bp = usage_fee_bp.as_bps();
        self
    }

    /// Set commitment fee using typed basis points.
    pub fn with_commitment_fee_bps(mut self, commitment_fee_bp: Bps) -> Self {
        self.commitment_fee_bp = commitment_fee_bp.as_bps();
        self
    }
}

/// Margin step-up event (covenant penalty or scheduled increase).
///
/// Increases the interest margin by a fixed amount at a specified date,
/// typically triggered by covenant breach or scheduled rating migration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MarginStepUp {
    /// Effective date of margin increase
    #[schemars(with = "String")]
    pub date: Date,
    /// Increase in margin (basis points)
    pub delta_bp: i32,
}

impl MarginStepUp {
    /// Create a margin step-up using typed basis points.
    pub fn new_bps(date: Date, delta_bp: Bps) -> Self {
        Self {
            date,
            delta_bp: delta_bp.as_bps(),
        }
    }
}

/// Payment-in-kind (PIK) toggle event.
///
/// Enables or disables PIK interest at a specified date. When enabled,
/// a portion of interest may be capitalized rather than paid in cash.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PikToggle {
    /// Date PIK feature is toggled
    #[schemars(with = "String")]
    pub date: Date,
    /// True to enable PIK, false to disable
    pub enable_pik: bool,
}

/// Cash sweep event (mandatory prepayment from excess cash flow).
///
/// Represents scheduled or covenant-triggered prepayment from borrower's
/// excess cash flow, reducing outstanding principal.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CashSweepEvent {
    /// Date of cash sweep prepayment
    #[schemars(with = "String")]
    pub date: Date,
    /// Amount of mandatory prepayment
    pub amount: Money,
}

/// Covenant-driven events for term loans.
///
/// Aggregates all covenant-triggered or scheduled events that modify
/// loan terms, including margin increases, PIK toggles, cash sweeps,
/// and draw restrictions.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TermLoanCovenantEvents {
    /// Margin step-up schedule
    pub margin_stepups: Vec<MarginStepUp>,
    /// PIK toggle schedule
    pub pik_toggles: Vec<PikToggle>,
    /// Cash sweep (mandatory prepayment) schedule
    pub cash_sweeps: Vec<CashSweepEvent>,
    /// Dates on which draws are prohibited (covenant breach or scheduled)
    #[schemars(with = "Vec<String>")]
    pub draw_stop_dates: Vec<Date>,
}

/// Principal amortization schedule specification.
///
/// Defines how the loan principal is amortized over its life,
/// from no amortization (bullet) to custom schedules.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub enum AmortizationSpec {
    /// Bullet loan with no scheduled amortization
    None,
    /// Linear amortization between start and end dates
    Linear {
        /// Amortization start date
        #[schemars(with = "String")]
        start: Date,
        /// Amortization end date (full repayment)
        #[schemars(with = "String")]
        end: Date,
    },
    /// Percentage of current outstanding principal per period (geometric decay).
    ///
    /// Each period, the amortization amount equals `bp / 10000 × current_outstanding`.
    /// Because the percentage is applied to the declining balance, the dollar amount
    /// decreases geometrically each period.
    ///
    /// **Note**: This is NOT the same as a flat percentage of original notional
    /// (which would produce equal dollar payments each period).  For example,
    /// 250 bp (2.5%) per quarter applied to $10M produces:
    /// - Q1: $250,000 (2.5% × $10M)
    /// - Q2: $243,750 (2.5% × $9.75M)
    /// - Q3: $237,656 (2.5% × $9.506M)
    /// - etc.
    PercentPerPeriod {
        /// Percentage in basis points per payment period (applied to current outstanding)
        bp: i32,
    },
    /// Flat dollar amortization each period (percentage of original notional).
    ///
    /// Each period, the amortization amount equals `bp / 10000 × original_notional`.
    /// Because the percentage is applied to the fixed original balance, the dollar
    /// amount is identical every period (unlike `PercentPerPeriod` which decays).
    ///
    /// For example, 250 bp (2.5%) per quarter applied to $10M produces:
    /// - Q1: $250,000 (2.5% × $10M)
    /// - Q2: $250,000 (2.5% × $10M)
    /// - Q3: $250,000 (2.5% × $10M)
    /// - etc.
    PercentOfOriginalNotional {
        /// Percentage in basis points per payment period (applied to original notional)
        bp: i32,
    },
    /// Custom amortization schedule with explicit principal payments
    Custom(#[schemars(with = "Vec<(String, Money)>")] Vec<(Date, Money)>),
}

impl AmortizationSpec {
    /// Create a per-period amortization schedule using typed basis points.
    pub fn percent_per_period_bps(bp: Bps) -> Self {
        Self::PercentPerPeriod { bp: bp.as_bps() }
    }
}

/// Complete term loan specification with covenant and DDTL features.
///
/// Comprehensive specification for institutional term loans including:
/// - Amortization schedules (bullet, linear, custom)
/// - Delayed-draw capabilities with commitment fees
/// - Payment-in-kind (PIK) features
/// - Covenant-driven events (margin step-ups, cash sweeps)
/// - Original issue discount (OID) handling
/// - Optional borrower callability
///
/// # Examples
///
/// ```text
/// # // Convert to runtime instrument via `try_into()` when needed.
/// use finstack_valuations::instruments::fixed_income::term_loan::spec::*;
/// use finstack_valuations::instruments::fixed_income::term_loan::types::RateSpec;
/// use finstack_valuations::instruments::pricing_overrides::PricingOverrides;
/// use finstack_cashflows::builder::specs::CouponType;
/// use finstack_cashflows::builder::FloatingRateSpec;
/// use finstack_core::money::Money;
/// use finstack_core::currency::Currency;
/// use finstack_core::dates::*;
/// use finstack_core::types::{InstrumentId, CurveId};
/// use rust_decimal_macros::dec;
/// use time::Month;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Example: Floating-rate term loan with SOFR + 300 bps
/// let floating_spec = FloatingRateSpec {
///     index_id: CurveId::new("USD-SOFR-3M"),
///     spread_bp: dec!(300),
///     gearing: dec!(1),
///     gearing_includes_spread: true,
///     index_floor_bp: Some(dec!(0)),  // 0% floor
///     all_in_floor_bp: None,
///     all_in_cap_bp: None,
///     index_cap_bp: None,
///     reset_freq: Tenor::quarterly(),
///     reset_lag_days: 2,
///     dc: DayCount::Act360,
///     bdc: BusinessDayConvention::ModifiedFollowing,
///     calendar_id: None,
///     fixing_calendar_id: None,
/// };
///
/// let spec = TermLoanSpec {
///     id: InstrumentId::new("TL-001"),
///     discount_curve_id: CurveId::new("USD-CREDIT"),
///     currency: Currency::USD,
///     notional_limit: Some(Money::new(25_000_000.0, Currency::USD)),
///     issue: create_date(2025, Month::January, 15)?,
///     maturity: create_date(2030, Month::January, 15)?,
///     rate: RateSpec::Floating(floating_spec),
///     frequency: Tenor::quarterly(),
///     day_count: DayCount::Act360,
///     bdc: BusinessDayConvention::ModifiedFollowing,
///     calendar_id: None,
///     stub: StubKind::None,
///     amortization: AmortizationSpec::None,  // Bullet loan
///     coupon_type: CouponType::Cash,
///     upfront_fee: None,
///     ddtl: None,
///     covenants: None,
///     credit_curve_id: None,
///     pricing_overrides: PricingOverrides::default(),
///     oid_eir: None,
///     call_schedule: None,
///     settlement_days: 2,
/// };
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TermLoanSpec {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Discount curve ID for present value calculations
    pub discount_curve_id: CurveId,
    /// Optional credit curve ID for hazard rate / credit risk calculations.
    ///
    /// If not provided, defaults to `discount_curve_id` (risky discounting).
    pub credit_curve_id: Option<CurveId>,
    /// Loan currency
    pub currency: Currency,
    /// Maximum commitment / notional limit.
    ///
    /// If omitted and `ddtl` is provided, the commitment limit is used.
    /// Required for non-DDTL term loans.
    pub notional_limit: Option<Money>,
    /// Loan issue/origination date
    #[schemars(with = "String")]
    pub issue: Date,
    /// Final maturity date
    #[schemars(with = "String")]
    pub maturity: Date,
    /// Interest rate specification (fixed or floating)
    pub rate: RateSpec,
    /// Payment frequency for interest and principal
    pub frequency: Tenor,
    /// Day count convention for interest accrual
    pub day_count: DayCount,
    /// Business day convention for schedule adjustment
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub bdc: BusinessDayConvention,
    /// Optional holiday calendar ID (default: no holidays)
    pub calendar_id: Option<String>,
    /// Stub period treatment
    #[serde(default = "crate::serde_defaults::stub_short_front")]
    pub stub: StubKind,
    /// Principal amortization schedule
    pub amortization: AmortizationSpec,
    /// Coupon characterization (Cash, PIK, or Split with optional toggles).
    ///
    /// This field controls whether interest is paid in cash, capitalized (PIK),
    /// or split between the two. It does NOT control payment timing (which is
    /// assumed to be in arrears). For dynamic PIK toggles, see `TermLoanCovenantEvents::pik_toggles`.
    pub coupon_type: crate::cashflow::builder::specs::CouponType,
    /// Optional upfront origination fee
    pub upfront_fee: Option<Money>,
    /// Optional delayed-draw term loan features
    pub ddtl: Option<DdtlSpec>,
    /// Optional covenant-driven events
    pub covenants: Option<TermLoanCovenantEvents>,
    /// Pricing overrides (yield, price, etc.)
    pub pricing_overrides: PricingOverrides,
    /// Optional EIR amortization settings for reporting schedules
    #[serde(default)]
    pub oid_eir: Option<OidEirSpec>,
    /// Optional call schedule (borrower callability)
    pub call_schedule: Option<LoanCallSchedule>,
    /// Settlement days (T+n). Default is 2 for leveraged loans per LSTA conventions.
    ///
    /// LSTA standard for secondary market loan trades is T+2 (effective since 2023).
    /// Primary market trades may use different conventions.
    #[serde(default = "default_settlement_days")]
    pub settlement_days: u32,
}

fn default_settlement_days() -> u32 {
    2
}

/// Type of borrower call provision on a term loan.
///
/// Institutional term loans use several types of call provisions:
/// - **Hard call**: Non-callable until the call date, then callable at the stated price.
/// - **Soft call**: Callable at any time, but subject to a premium (call protection).
///   Typically applies for the first 6-24 months ("non-call period").
/// - **Make-whole**: Borrower must pay the present value of remaining cashflows
///   discounted at a reference rate (typically a Treasury rate) plus a spread.
///   This ensures the lender receives full economic value upon early prepayment.
///
/// # Industry Practice
///
/// Leveraged term loans typically have 6-12 months of soft call protection
/// (101% of par, sometimes called "soft call 101"), after which they become
/// callable at par. Make-whole provisions are more common in investment-grade
/// term loans and private placements.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
#[derive(Default)]
pub enum LoanCallType {
    /// Hard call: callable at the stated price on or after the call date.
    /// This is the default if no call type is specified.
    #[default]
    Hard,
    /// Soft call: callable with a premium during the call protection period.
    /// After the protection period, callable at par.
    Soft,
    /// Make-whole call: borrower pays PV of remaining cashflows at a reference
    /// rate plus the specified spread. Ensures lender receives full economic value.
    MakeWhole {
        /// Spread over the reference rate in basis points (e.g., 50 = T+50bps).
        treasury_spread_bp: i32,
    },
}

/// Borrower call option on term loan.
///
/// Represents the borrower's right to prepay the loan at a specified
/// redemption price (typically at premium to par for early calls,
/// approaching par near maturity).
///
/// # Call Types
///
/// The `call_type` field determines how the call is exercised:
/// - `Hard`: Standard call at `price_pct_of_par` on or after `date`
/// - `Soft`: Premium call during protection period
/// - `MakeWhole`: PV-based redemption at Treasury + spread
///
/// For `MakeWhole` calls, `price_pct_of_par` serves as the minimum
/// (floor) redemption price. The actual price is the greater of
/// `price_pct_of_par` and the make-whole amount.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LoanCall {
    /// Call date (earliest prepayment date for this call provision)
    #[schemars(with = "String")]
    pub date: Date,
    /// Redemption price as percentage of par (e.g., 102.0 = 102% of par).
    /// For make-whole calls, this is the minimum (floor) price.
    pub price_pct_of_par: f64,
    /// Type of call provision. Defaults to `Hard` when unspecified.
    #[serde(default)]
    pub call_type: LoanCallType,
}

/// Complete call schedule for callable term loans.
///
/// Aggregates all borrower call provisions, typically with step-down
/// premiums as the loan ages (e.g., 103% in year 1, 102% in year 2, par thereafter).
///
/// # Examples
///
/// ```text
/// use finstack_valuations::instruments::fixed_income::term_loan::spec::{LoanCallSchedule, LoanCall};
/// use finstack_core::dates::create_date;
/// use time::Month;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let schedule = LoanCallSchedule {
///     calls: vec![
///         LoanCall {
///             date: create_date(2027, Month::January, 15)?,
///             price_pct_of_par: 103.0,  // 3% premium in year 2
///         },
///         LoanCall {
///             date: create_date(2028, Month::January, 15)?,
///             price_pct_of_par: 101.5,  // 1.5% premium in year 3
///         },
///         LoanCall {
///             date: create_date(2029, Month::January, 15)?,
///             price_pct_of_par: 100.0,  // At par thereafter
///         },
///     ],
/// };
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LoanCallSchedule {
    /// Ordered call provisions (typically sorted by date with descending premiums)
    pub calls: Vec<LoanCall>,
}
