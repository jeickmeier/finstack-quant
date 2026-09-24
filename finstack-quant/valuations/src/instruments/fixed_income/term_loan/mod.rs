//! Term loan instruments with covenant and delayed-draw features.
//!
//! This module provides modeling for institutional term loans including:
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
//! ```
//! use finstack_quant_valuations::instruments::fixed_income::term_loan::{
//!     AmortizationSpec, RateSpec, TermLoan,
//! };
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::dates::{DayCount, Tenor};
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::types::{CurveId, InstrumentId};
//! use time::macros::date;
//!
//! // Fixed-rate 6% bullet term loan; `build()` validates the contract.
//! let loan = TermLoan::builder()
//!     .id(InstrumentId::new("TL-BULLET-5Y"))
//!     .currency(Currency::USD)
//!     .notional_limit(Money::new(10_000_000.0, Currency::USD)?)
//!     .issue_date(date!(2025 - 01 - 15))
//!     .maturity(date!(2030 - 01 - 15))
//!     .rate(RateSpec::Fixed { rate_bp: 600 })
//!     .frequency(Tenor::quarterly())
//!     .day_count(DayCount::Act360)
//!     .discount_curve_id(CurveId::new("USD-CREDIT"))
//!     .amortization(AmortizationSpec::None)
//!     .attributes(Default::default())
//!     .build()?;
//! assert_eq!(loan.currency, Currency::USD);
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```
//!
//! # See Also
//!
//! - [`spec`] for the term-sheet component types (DDTL, covenants, amortization, calls)
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

pub use super::loan_terms::{CommitmentStep, MarginStepUp, OidEirSpec};
pub use overrides::TermLoanOverrides;
pub use spec::{
    AmortizationSpec, CashSweepEvent, CommitmentFeeBase, DdtlSpec, DrawEvent, LoanCall,
    LoanCallSchedule, LoanCallType, OidPolicy, PikToggle, TermLoanCovenantEvents,
};
pub use types::{RateSpec, TermLoan, TermLoanBuilder};

pub use pricing::TermLoanDiscountingPricer;
pub use pricing::TermLoanTreePricer;
