//! Credit Default Swap (CDS) instruments following ISDA standard model.
//!
//! Implements single-name CDS pricing and risk calculations conforming to
//! the ISDA CDS Standard Model v1.8.2 (October 2009). This is the industry-standard
//! methodology used by major dealers and clearing houses.
//!
//! Reported NPV defaults to Bloomberg CDSW clean
//! (`CdsValuationConvention::BloombergCdswClean`). `IsdaDirty` is opt-in
//! for academic ISDA Standard Model reproduction and is not the production quote.
//!
//! # CDS Structure
//!
//! A CDS is a bilateral contract providing credit protection:
//!
//! - **Protection seller**: Receives premium, pays (1-R)·Notional if default occurs
//! - **Protection buyer**: Pays premium, receives compensation upon default
//!
//! # Cashflow Legs
//!
//! ## Premium Leg (Paid by protection buyer)
//!
//! - Quarterly or semi-annual premium payments
//! - Premium = Spread × Notional × Day Count Fraction
//! - Stops at maturity or default (whichever comes first)
//! - **Accrual on default**: Partial premium from last payment to default
//!
//! ## Protection Leg (Paid by protection seller upon default)
//!
//! - Contingent payment = (1 - Recovery Rate) × Notional
//! - Paid only if default event occurs before maturity
//! - Present value accounts for default probability and timing
//!
//! # Pricing Model: ISDA CDS Standard Model
//!
//! ## Present Value of Premium Leg (with accrual-on-default)
//!
//! ```text
//! PV_prem = S · Σᵢ τᵢ · DF(tᵢ) · P(tᵢ) + S · Σᵢ ½·τᵢ · DF(tᵢ) · [P(tᵢ₋₁) - P(tᵢ)]
//! ```
//!
//! where:
//! - S = CDS spread (running coupon)
//! - τᵢ = day count fraction for period i
//! - DF(tᵢ) = discount factor to payment date i
//! - P(t) = survival probability to time t
//! - Second term = accrual on default
//!
//! ## Present Value of Protection Leg
//!
//! ```text
//! PV_prot = (1 - R) · Σᵢ DF(tᵢ) · [P(tᵢ₋₁) - P(tᵢ)]
//! ```
//!
//! where R = recovery rate (typically 40% for senior unsecured debt).
//!
//! ## CDS Par Spread
//!
//! The par spread S₀ that makes PV_CDS = 0:
//!
//! ```text
//! S₀ = (1 - R) · Σ DF(tᵢ)·[P(tᵢ₋₁) - P(tᵢ)] / Risky Annuity
//! ```
//!
//! # ISDA Conventions (Post-Big Bang 2009)
//!
//! ## Standard Terms
//! - **Trade Date**: Contract initiation date
//! - **Effective Date**: Protection starts (typically T+1)
//! - **Maturity**: IMM date (20th of Mar/Jun/Sep/Dec)
//! - **Coupon**: Standard running (100bp or 500bp in North America)
//!
//! ## Day Count
//! - **Premium leg**: ACT/360 (ISDA standard)
//! - **Protection**: Continuous (integrated over default probability)
//!
//! ## Business Day Convention
//! - Modified Following with calendar adjustments
//!
//! ## Settlement
//! - **Cash settlement**: Auction-determined recovery
//! - **Physical settlement**: Delivery of defaulted obligations
//!
//! # Academic and Industry References
//!
//! ## ISDA Standards (Primary Sources)
//!
//! - ISDA (2009). "ISDA CDS Standard Model." Version 1.8.2, October 2009.
//!   Available at: <https://www.cdsmodel.com/>
//!   (Industry-standard implementation)
//!
//! - ISDA (2009). "CDS Small Bang" Protocol. April 2009.
//!   (Standardized North American CDS terms)
//!
//! - ISDA (2009). "CDS Big Bang" Protocol. April 2009.
//!   (Introduction of standard coupons and upfront payments)
//!
//! - ISDA (2014). "ISDA 2014 Credit Derivatives Definitions."
//!   (Updated definitions post-financial crisis)
//!
//! ## Academic References
//!
//! - O'Kane, D., & Turnbull, S. (2003). "Valuation of Credit Default Swaps."
//!   *Fixed Income Quantitative Credit Research*, Lehman Brothers.
//!   (Foundational CDS pricing methodology) `docs/REFERENCES.md#o-kane-2008`
//!
//! - Hull, J. C., & White, A. (2000). "Valuing Credit Default Swaps I: No
//!   Counterparty Default Risk." *Journal of Derivatives*, 8(1), 29-40. `docs/REFERENCES.md#hull-white-2000-cds`
//!
//! - Duffie, D. (1999). "Credit Swap Valuation." *Financial Analysts Journal*,
//!   55(1), 73-87.
//!
//! - O'Kane, D. (2008). *Modelling Single-name and Multi-name Credit Derivatives*.
//!   Wiley Finance.
//!   (Comprehensive reference for CDS pricing and risk) `docs/REFERENCES.md#o-kane-2008`
//!
//! # Implementation Notes
//!
//! - Protection and accrual-on-default legs use analytical piecewise-constant
//!   integration over a grid containing hazard- and discount-curve knots plus
//!   configured intra-knot substeps. Default cashflows are discounted to
//!   settlement dates resolved from the protection-leg delay and calendar.
//! - Valuation consumes deterministic hazard, discount, and recovery inputs;
//!   stochastic credit/recovery, default correlation, and CDS
//!   quanto/currency-basis effects are not modeled.
//!
//! # Examples
//!
//! See [`CreditDefaultSwap`] for construction and usage examples.
//!
//! # See Also
//!
//! - `CreditDefaultSwap` for single-name CDS struct
//! - `CdsConvention` for regional standard conventions
//! - `PremiumLegSpec` for premium leg configuration
//! - `ProtectionLegSpec` for protection leg configuration
//! - CDS metrics module for CDS risk metrics (CS01, DV01, recovery sensitivity)

pub(crate) mod metrics;
pub(crate) mod pricing;
mod types;

pub use crate::market::conventions::ids::CdsDocClause;
pub use crate::market::conventions::CdsConvention;
pub use crate::market::conventions::CdsValuationConvention;
pub use types::CreditDefaultSwap;
pub use types::CreditDefaultSwapBuilder;
pub use types::PayReceive;
pub use types::PremiumLegSpec;
pub use types::ProtectionLegSpec;
