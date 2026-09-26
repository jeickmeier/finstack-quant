//! Serde-stable specification types for term loans and DDTL features.
//!
//! This module defines the serializable term-sheet components a
//! [`TermLoan`](super::types::TermLoan) composes: delayed-draw term loan
//! (DDTL) features, covenant events, amortization schedules and call
//! provisions. The loan itself is built with `TermLoan::builder()` (or
//! deserialized directly), which validates the complete contract.
//!
//! # Overview
//!
//! All types in this module are designed for stable serialization with
//! `#[serde(deny_unknown_fields)]` to catch configuration errors and explicit
//! field naming for long-lived pipelines.
//!
//! # Key Types
//!
//! - [`DdtlSpec`]: Delayed-draw term loan features
//! - [`TermLoanCovenantEvents`]: Covenant-driven events
//! - [`AmortizationSpec`]: Principal repayment schedules (the shared cashflows
//!   type; term loans accept every variant except `LinearTo` and
//!   `StepRemaining`)
//! - [`LoanCallSchedule`]: Borrower prepayment options
//! - [`OidPolicy`]: Original issue discount handling
//! - [`OidEirSpec`]: Effective interest rate amortization settings
//!
//! LSTA par-trade marks use T+7:
//!
//! ```
//! # use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
//! # fn example() -> finstack_quant_core::Result<()> {
//! let mut loan = TermLoan::example()?;
//! loan.settlement_days = 7;
//! # let _ = loan;
//! # Ok(())
//! # }
//! ```
//!
//! # See Also
//!
//! - [`super::types::TermLoan`] for the runtime instrument type
//! - `super::cashflows` for cashflow generation (internal module)
//! - term loan pricing module for valuation

use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use rust_decimal::Decimal;

pub use super::super::bond::MakeWholeSpec;
pub use super::super::loan_terms::{CommitmentStep, DrawEvent, MarginStep, OidEirSpec};
pub use crate::cashflow::builder::AmortizationSpec;

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
/// Typical OID ranges from 1-5% (100-500 bp) of par value.
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
/// - `WithheldBp`: Discount in basis points of each funded draw, withheld
///   from proceeds
/// - `WithheldAmount`: Fixed facility-level amount withheld from funded
///   proceeds, pro-rated across draws by draw size
/// - `SeparateBp`: Discount in basis points of each draw, tracked
///   separately and not withheld
/// - `SeparateAmount`: Fixed facility-level amount tracked separately,
///   pro-rated across draws by draw size
///
/// # Examples
///
/// ```text
/// use finstack_quant_valuations::instruments::fixed_income::term_loan::spec::OidPolicy;
/// use finstack_quant_core::money::Money;
/// use finstack_quant_core::currency::Currency;
///
/// // 2% OID withheld from proceeds
/// let oid = OidPolicy::WithheldBp(dec!(200));  // 200 bp = 2%
///
/// // $50,000 fixed OID
/// let oid_fixed = OidPolicy::WithheldAmount(Money::from((50_000_i64, Currency::USD)));
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum OidPolicy {
    /// Discount in basis points of each funded draw (`200` = 2%), withheld
    /// from proceeds
    WithheldBp(
        #[serde(with = "finstack_quant_core::wire::decimal")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DecimalWire")
        )]
        Decimal,
    ),
    /// Fixed facility-level discount amount withheld from funded proceeds,
    /// pro-rated across draws by draw size
    WithheldAmount(Money),
    /// Discount in basis points of each draw (`200` = 2%), tracked
    /// separately for amortization
    SeparateBp(
        #[serde(with = "finstack_quant_core::wire::decimal")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DecimalWire")
        )]
        Decimal,
    ),
    /// Fixed facility-level discount amount tracked separately for
    /// amortization, pro-rated across draws by draw size
    SeparateAmount(Money),
}

/// Basis for calculating commitment fees on undrawn portions.
///
/// Determines the denominator for commitment fee calculations on
/// revolving or delayed-draw facilities.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
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
/// - Commitment fees: 25-50 bp on undrawn amounts
/// - Usage fees: 0-25 bp on drawn amounts
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
/// use finstack_quant_valuations::instruments::fixed_income::term_loan::spec::*;
/// use finstack_quant_core::money::Money;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::dates::create_date;
/// use time::Month;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let ddtl = DdtlSpec {
///     commitment: Money::from((10_000_000_i64, Currency::USD)),
///     availability_start: create_date(2025, Month::January, 1)?,
///     availability_end: create_date(2026, Month::January, 1)?,
///     draws: vec![],
///     commitment_steps: vec![],
///     usage_fee_bp: dec!(50),        // 50 bp usage fee
///     commitment_fee_bp: dec!(25),   // 25 bp commitment fee
///     fee_base: CommitmentFeeBase::Undrawn,
///     oid_policy: None,
/// };
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DdtlSpec {
    /// Total commitment available for draws, in the loan currency.
    pub commitment: Money,
    /// First date draws are permitted
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub availability_start: Date,
    /// Last date draws are permitted (commitment expiry)
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub availability_end: Date,
    /// Scheduled or actual draw events
    pub draws: Vec<DrawEvent>,
    /// Commitment steps, each effective from its date: strictly increasing
    /// dates inside the availability window, non-increasing `amount`s in the
    /// loan currency, and a zero `reduction_fee_bp` (term loans carry no
    /// reduction fee).
    pub commitment_steps: Vec<CommitmentStep>,
    /// Usage fee on drawn amounts, in basis points per annum (non-negative;
    /// `25` = 0.25%).
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub usage_fee_bp: Decimal,
    /// Commitment fee on the undrawn commitment, in basis points per annum
    /// (non-negative; `50` = 0.50%).
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub commitment_fee_bp: Decimal,
    /// Basis for commitment fee calculation
    pub fee_base: CommitmentFeeBase,
    /// Original issue discount policy, if applicable
    pub oid_policy: Option<OidPolicy>,
}

impl DdtlSpec {
    /// Commitment in force on `date`: the last step dated on or before
    /// `date`, else `commitment`.
    ///
    /// # Arguments
    ///
    /// * `date` - Date the limit is wanted for; a step dated on it applies.
    pub(crate) fn limit_in_force_at(&self, date: Date) -> Money {
        self.commitment_steps
            .iter()
            .filter(|step| step.date <= date)
            .map(|step| step.amount)
            .next_back()
            .unwrap_or(self.commitment)
    }
}

/// Payment-in-kind (PIK) toggle event.
///
/// Enables or disables PIK interest at a specified date. When enabled,
/// a portion of interest may be capitalized rather than paid in cash.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PikToggle {
    /// Date PIK feature is toggled
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// True to enable PIK, false to disable
    pub enable_pik: bool,
}

/// Cash sweep event (mandatory prepayment from excess cash flow).
///
/// Represents scheduled or covenant-triggered prepayment from borrower's
/// excess cash flow, reducing outstanding principal.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CashSweepEvent {
    /// Date of cash sweep prepayment
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Amount of mandatory prepayment
    pub amount: Money,
}

/// Covenant-driven events for term loans.
///
/// Aggregates all covenant-triggered or scheduled events that modify
/// loan terms, including margin increases, PIK toggles, cash sweeps,
/// and draw restrictions.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TermLoanCovenantEvents {
    /// Margin steps, each a non-negative cumulative bp change effective from
    /// the start of the first interest period on or after its date.
    pub margin_steps: Vec<MarginStep>,
    /// PIK toggle schedule
    pub pik_toggles: Vec<PikToggle>,
    /// Cash sweep (mandatory prepayment) schedule
    pub cash_sweeps: Vec<CashSweepEvent>,
    /// Dates on which draws are prohibited (covenant breach or scheduled)
    #[serde(with = "finstack_quant_core::wire::dates")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    pub draw_stop_dates: Vec<Date>,
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
#[derive(Default)]
#[serde(rename_all = "snake_case")]
pub enum LoanCallType {
    /// Hard call: callable at the stated price on or after the call date.
    /// This is the default if no call type is specified.
    #[default]
    Hard,
    /// Soft call: callable with a premium during the call protection period.
    /// After the protection period, callable at par.
    Soft,
    /// Make-whole call: borrower pays PV of remaining cashflows at the
    /// reference curve plus spread of the shared [`MakeWholeSpec`] (the same
    /// type bonds use). The term-loan tree pricer and yield-to-call metrics
    /// never exercise it: paying at least the make-whole amount leaves the
    /// lender indifferent, so the loan is not economically callable.
    MakeWhole(MakeWholeSpec),
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
/// - `MakeWhole`: PV-based redemption at the reference curve plus spread
///
/// For `MakeWhole` calls, `price_pct_of_par` serves as the minimum
/// (floor) redemption price. The actual price is the greater of
/// `price_pct_of_par` and the make-whole amount.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LoanCall {
    /// Call date (earliest prepayment date for this call provision)
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
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
/// use finstack_quant_valuations::instruments::fixed_income::term_loan::spec::{LoanCallSchedule, LoanCall};
/// use finstack_quant_core::dates::create_date;
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
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LoanCallSchedule {
    /// Ordered call provisions (typically sorted by date with descending premiums)
    pub calls: Vec<LoanCall>,
}
