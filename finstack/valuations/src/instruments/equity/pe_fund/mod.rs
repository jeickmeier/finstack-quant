//! Private markets fund modeling with waterfall distributions.
//!
//! Models private equity and private credit funds with:
//! - Multi-period NAV evolution
//! - Tiered waterfall distributions (GP/LP splits)
//! - Hurdle rates and catch-up provisions
//! - Carried interest calculations
//! - Management and performance fees
//!
//! # Fund Structure
//!
//! Typical private fund waterfall:
//! 1. **Return of capital**: LPs receive initial investments back
//! 2. **Preferred return**: LPs receive hurdle rate (e.g., 8% IRR)
//! 3. **Catch-up**: GP receives distributions until target carry achieved
//! 4. **Carried interest**: Profits split (typically 80/20 LP/GP)
//!
//! # Pricing
//!
//! Private funds are valued by discounting projected NAV distributions:
//!
//! ```text
//! PV = Σ Distribution(t) · DF(t)
//! ```
//!
//! where distributions follow the contractual waterfall based on NAV evolution.
//!
//! # Key Metrics
//!
//! - **NAV**: Net Asset Value (fund portfolio value)
//! - **DPI**: Distributions to Paid-In capital ratio
//! - **TVPI**: Total Value to Paid-In (DPI + RVPI)
//! - **IRR**: Internal Rate of Return
//! - **NAV01**: Sensitivity to NAV changes
//! - **Carry01**: Sensitivity to carried interest rate
//!
//! # See Also
//!
//! - [`PrivateMarketsFund`] for fund struct
//! - private markets waterfall module for distribution waterfall logic
//! - private markets metrics module for private markets metrics

pub(crate) mod metrics;
/// Private markets fund pricer implementation
pub(crate) mod pricer;
mod types;
pub mod waterfall;

#[doc(hidden)]
pub use metrics::{
    calculate_irr, CarryAccruedCalculator, DpiLpCalculator, GpIrrCalculator, LpIrrCalculator,
    MoicLpCalculator, TvpiLpCalculator,
};
pub use types::PrivateMarketsFund;
pub use waterfall::{
    AllocationLedger, AllocationRow, CatchUpMode, ClawbackSettle, ClawbackSpec,
    EquityWaterfallEngine, FundEvent, FundEventKind, Hurdle, Tranche, WaterfallSpec,
    WaterfallSpecBuilder, WaterfallStyle,
};
