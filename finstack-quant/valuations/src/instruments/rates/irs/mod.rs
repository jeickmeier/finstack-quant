//! Interest rate swap instruments and pricing.
//!
//! Interest rate swaps (IRS) are OTC derivatives where two parties exchange
//! fixed and floating interest rate cashflows on a notional amount. This module
//! provides plain vanilla swaps, basis swaps, and par rate calculations.
//!
//! # Swap Structure
//!
//! A standard "payer" swap:
//! - **Fixed leg**: Pay fixed rate on notional
//! - **Float leg**: Receive floating rate (e.g., SOFR, EURIBOR)
//!
//! A "receiver" swap is the opposite (receive fixed, pay floating).
//!
//! # Pricing
//!
//! Under the risk-neutral measure, swap value is:
//!
//! ```text
//! PV_swap = PV_fixed_leg - PV_float_leg
//! ```
//!
//! For a payer swap:
//! ```text
//! PV = PV_float - PV_fixed
//!    = N · Σ τᵢ · Fwd(t_i) · DF(t_i) - N · K · Σ τᵢ · DF(t_i)
//! ```
//!
//! where:
//! - N = notional
//! - K = fixed rate
//! - Fwd(t_i) = forward rate for period i
//! - DF(t_i) = discount factor to payment date i
//! - τᵢ = accrual period (day count fraction)
//!
//! # Par Swap Rate
//!
//! The par rate is the fixed rate that makes PV_swap = 0:
//!
//! ```text
//! Par Rate = Σ τᵢ · Fwd(t_i) · DF(t_i) / Σ τᵢ · DF(t_i)
//!          = (DF(start) - DF(end)) / Annuity
//! ```
//!
//! # Market Conventions
//!
//! Standard conventions by currency:
//!
//! - **USD**: ACT/360 (float), ACT/360 (fixed OIS), SOFR index
//! - **EUR**: ACT/360 (float), ACT/360 (fixed OIS), €STR index
//! - **GBP**: ACT/365F (float), ACT/365F (fixed), SONIA index
//! - **JPY**: ACT/365F (float), ACT/365F (fixed), TONA index
//! - **CAD**: ACT/365F (float), ACT/365F (fixed), CORRA (OIS) index
//! - **AUD**: ACT/365F (float), ACT/365F (fixed), AONIA / BBSW index
//! - **NZD**: ACT/365F (float), ACT/365F (fixed), BKBM index
//! - **CHF**: ACT/360 (float), ACT/360 (fixed OIS), SARON index
//! - **CNY**: ACT/365F (float), ACT/365F (fixed), Shibor index
//!
//! # Key Metrics
//!
//! - **Par Rate**: Market swap rate for zero initial value
//! - **DV01**: Dollar value of 1bp parallel shift in curve
//! - **Bucketed DV01**: Sensitivity to individual curve points
//! - **Annuity**: Present value of 1 unit paid each period
//!
//! # References
//!
//! ## Academic & Industry Standards
//!
//! - **ISDA 2021 Definitions**: Current interest-rate derivative framework,
//!   including RFR compounding and modern floating-rate options
//!   `docs/REFERENCES.md#isda-2021-definitions`
//! - **ISDA 2006 Definitions**: Legacy transaction conventions retained for
//!   historical contracts `docs/REFERENCES.md#isda-2006-definitions`
//! - Hull, J. C. (2018). *Options, Futures, and Other Derivatives* (10th ed.).
//!   Pearson. Chapter 7: Swaps. `docs/REFERENCES.md#hull-options-futures`
//! - Tuckman, B., & Serrat, A. (2011). *Fixed Income Securities: Tools for
//!   Today's Markets* (3rd ed.). Wiley. Chapters 3-4: Swaps and Duration. `docs/REFERENCES.md#tuckman-serrat-fixed-income`
//! - Brigo, D., & Mercurio, F. (2006). *Interest Rate Models - Theory and
//!   Practice* (2nd ed.). Springer Finance. Chapter 1: Definitions and
//!   Conventions. `docs/REFERENCES.md#brigo-mercurio-2006-interest-rate-models`
//!
//! # Examples
//!
//! See [`InterestRateSwap`] for construction and usage examples.
//!
//! # See Also
//!
//! - [`InterestRateSwap`] for the main swap struct
//! - `FixedLegSpec` for fixed leg specification
//! - `FloatLegSpec` for floating leg specification
//! - `PayReceive` for swap direction
//! - swap metrics module for swap-specific risk metrics

pub(crate) mod cashflow;
pub mod compounding;
pub(crate) mod metrics;
/// Interest rate swap pricer implementation
pub(crate) mod pricer;
mod types;

pub use compounding::FloatingLegCompounding;
pub use types::{
    ConventionSwapParams, FixedLegSpec, FloatLegSpec, InterestRateSwap, InterestRateSwapBuilder,
    IrsLegConventions, ParRateMethod, PayReceive,
};

/// Error for a swap float leg set to `simple_average`, which the swap
/// projectors do not implement (they compound or take one term forward).
pub(crate) fn simple_average_unsupported() -> finstack_quant_core::Error {
    finstack_quant_core::Error::Validation(
        "float_leg.compounding = simple_average is not supported on swap legs; \
         use simple or a compounded_* variant"
            .to_string(),
    )
}
