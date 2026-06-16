//! Exchange-traded options on interest rate futures.
//!
//! Options on IR futures (e.g., SOFR futures options) are among the most liquid
//! interest rate derivatives. They are priced using the Black-76 model with the
//! futures price as the forward (no convexity adjustment needed).
//!
//! # Contract Types
//!
//! - **SOFR futures options**: Options on 1M and 3M SOFR futures (CME)
//! - **Eurodollar options**: Historical options on ED futures (now SOFR)
//! - **Euribor options**: Options on 3M EURIBOR futures (ICE)
//!
//! # Price Convention
//!
//! Options quote in price points (e.g., 0.25 = 25 ticks). The underlying
//! futures quote as 100 minus rate, so a call benefits from falling rates.
//!
//! # Exercise Style
//!
//! Most exchange-listed IR futures options are American-style, but early exercise
//! is rarely optimal because futures options have zero carrying cost. European
//! pricing via Black-76 is standard market practice.
//!
//! # References
//!
//! - Black, F. (1976). "The pricing of commodity contracts."
//!   *Journal of Financial Economics*, 3(1-2), 167-179.
//!
//! - Hull, J. C. (2018). *Options, Futures, and Other Derivatives* (10th ed.).
//!   Pearson. Chapter 18: Options on Futures.

pub(crate) mod metrics;
pub(crate) mod pricer;
mod types;

pub use types::IrFutureOption;
