//! Term loan instruments with covenant and delayed-draw features.
//!
//! This module provides comprehensive modeling for institutional term loans including:
//! - Standard term loans with fixed or floating rates
//! - Delayed-draw term loans (DDTL) with commitment periods and fees
//! - Payment-in-kind (PIK) interest with toggles
//! - Amortization schedules (bullet, linear, custom)
//! - Covenant-driven events (margin step-ups, cash sweeps, draw restrictions)
//! - Original issue discount (OID) handling
//! - Borrower callability schedules
//!
//! # Features
//!
//! - **Multiple rate types**: Fixed rate or floating with floors/caps/gearing
//! - **DDTL capabilities**: Draw schedules, commitment fees, usage fees, step-downs
//! - **Flexible amortization**: Bullet, linear, percent-per-period, or custom schedules
//! - **PIK support**: Full PIK, cash-only, or split coupons with covenant toggles
//! - **Covenant events**: Margin step-ups, cash sweeps, PIK toggles, draw restrictions
//! - **OID policies**: Withheld or separate tracking for discount amortization
//! - **Callability**: Step-down premium schedules for borrower prepayment options
//! - **Deterministic pricing**: Full cashflow generation with discounting
//! - **Yield metrics**: YTM, YTC, YTW, YT2Y/3Y/4Y, All-In Rate, Discount Margin
//! - **Risk metrics**: DV01, CS01, Theta with bucketed support
//!
//! # Quick Example
//!
//! ```ignore
//! use finstack_valuations::instruments::fixed_income::term_loan::{TermLoan, TermLoanSpec, RateSpec};
//! use finstack_valuations::instruments::fixed_income::term_loan::spec::AmortizationSpec;
//! use finstack_cashflows::builder::specs::CouponType;
//! use finstack_valuations::instruments::pricing_overrides::PricingOverrides;
//! use finstack_core::money::Money;
//! use finstack_core::currency::Currency;
//! use finstack_core::dates::*;
//! use finstack_core::types::{InstrumentId, CurveId};
//! use time::Month;
//! // Convert `TermLoanSpec` to `TermLoan` via `try_into()` when needed.
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Fixed-rate bullet term loan
//! let spec = TermLoanSpec {
//!     id: InstrumentId::new("TL-BULLET-5Y"),
//!     discount_curve_id: CurveId::new("USD-CREDIT"),
//!     currency: Currency::USD,
//!     notional_limit: Some(Money::new(10_000_000.0, Currency::USD)),
//!     issue: create_date(2025, Month::January, 15)?,
//!     maturity: create_date(2030, Month::January, 15)?,
//!     rate: RateSpec::Fixed { rate_bp: 600 },  // 6% fixed
//!     frequency: Tenor::quarterly(),
//!     day_count: DayCount::Act360,
//!     bdc: BusinessDayConvention::ModifiedFollowing,
//!     calendar_id: None,
//!     stub: StubKind::None,
//!     amortization: AmortizationSpec::None,  // Bullet
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
//!
//! let loan: TermLoan = spec.try_into()?;
//! # let _ = loan;
//! # Ok(())
//! # }
//! ```
//!
//! # See Also
//!
//! - [`crate::instruments::fixed_income::term_loan::TermLoanSpec`] for complete specification structure
//! - [`TermLoan`] for the instrument type
//! - term loan cashflows module for cashflow generation details
//! - term loan pricing module for valuation methodology
//! - term loan metrics module for available metrics

pub(crate) mod cashflows;
pub(crate) mod metrics;
pub mod overrides;
pub(crate) mod pricing;
pub mod spec;
pub(crate) mod types;

// Re-export main type
pub use overrides::TermLoanOverrides;
pub use spec::{
    AmortizationSpec, CashSweepEvent, CommitmentFeeBase, CommitmentStepDown, DdtlSpec, DrawEvent,
    LoanCall, LoanCallSchedule, LoanCallType, OidEirSpec, OidPolicy, PikToggle,
    TermLoanCovenantEvents, TermLoanSpec,
};
pub use types::{RateSpec, TermLoan};

// Re-export pricer for test access.
pub use pricing::TermLoanDiscountingPricer;
pub use pricing::TermLoanTreePricer;
