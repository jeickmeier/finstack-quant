//! Interest rate futures with convexity adjustments for curve calibration.
//!
//! Interest rate futures are exchange-traded contracts on future interest rates.
//! Major contracts include SOFR futures, Eurodollar futures, and Short Sterling
//! futures. They require convexity adjustments when used for curve calibration.
//!
//! # Contract Types
//!
//! - **SOFR futures**: 3-month SOFR rate (CME)
//! - **Eurodollar futures**: 3-month USD LIBOR (historical, now SOFR)
//! - **Short Sterling**: 3-month GBP LIBOR (historical, now SONIA)
//! - **Euribor futures**: 3-month EUR EURIBOR
//!
//! # Price Convention
//!
//! Futures quote = 100 - Implied Rate (in %)
//!
//! Example: Price 99.50 implies 0.50% rate
//!
//! # Convexity Adjustment
//!
//! Futures rates differ from forward rates due to daily marking:
//!
//! ```text
//! Forward_rate = Futures_rate - Convexity_adjustment
//! ```
//!
//! where convexity adjustment depends on rate volatility and time to expiry.
//!
//! Typical adjustment: 1-5 basis points for nearby contracts, larger for deferred.
//!
//! # Pricing
//!
//! Present value of futures position:
//!
//! ```text
//! PV = Position_sign × Contracts × Multiplier × (Price_mark - Price_entry)
//! ```
//!
//! where `Multiplier` is the currency value of one full price point
//! (`tick_value / tick_size`, $2,500 for CME SR3).
//!
//! For calibration, futures imply forward rates that price instruments.
//!
//! # Market Conventions
//!
//! - **Contract size**: $1,000,000 (SOFR), $1,000,000 (Eurodollar)
//! - **Multiplier**: $2,500 per price point for 3M SOFR ($25 per basis point)
//! - **Expiry**: IMM dates (3rd Wednesday of Mar/Jun/Sep/Dec)
//! - **Settlement**: Cash-settled to reference rate
//!
//! # References
//!
//! - Burghardt, G., & Hoskins, B. (1995). "A Question of Bias." *Risk Magazine*,
//!   8(3), 63-70. (Convexity bias in futures)
//!
//! - Hull, J. C. (2018). *Options, Futures, and Other Derivatives* (10th ed.).
//!   Pearson. Chapter 6: Interest Rate Futures. `docs/REFERENCES.md#hull-options-futures`
//!
//! # See Also
//!
//! - [`InterestRateFuture`] for instrument struct
//! - [`crate::instruments::rates::ir_future::FutureContractSpecs`] for contract specifications
//! - Plan-driven calibration in `calibration::api` (Forward step)

pub(crate) mod metrics;
mod types;

pub use types::{FutureContractSpecs, InterestRateFuture, RateAveragingMethod};

// Builder provided by FinancialBuilder derive
