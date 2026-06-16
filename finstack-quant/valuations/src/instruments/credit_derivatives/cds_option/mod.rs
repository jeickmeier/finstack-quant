//! CDS options (credit swaptions) priced under the Bloomberg CDSO
//! numerical-quadrature model.
//!
//! CDS options provide the right to buy or sell credit protection at a
//! predetermined spread. Also called credit swaptions, they are key
//! instruments for managing credit volatility.
//!
//! # Structure
//!
//! - **Payer option** (`Call`): right to buy protection (pay spread, receive on default)
//! - **Receiver option** (`Put`): right to sell protection (receive spread, pay on default)
//! - **Underlying**: single-name CDS or CDS index
//! - **Strike**: CDS spread level (decimal rate, e.g. `0.01` = 100 bp)
//!
//! # Pricing model: Bloomberg CDSO numerical quadrature
//!
//! Bloomberg's CDSO terminal prices both index and single-name CDS
//! options under a numerical-quadrature engine that integrates the
//! exercise payoff against a lognormal forward-spread distribution,
//! with the lognormal mean calibrated to the no-knockout forward
//! present value at expiry (DOCS 2055833 §1.2). The Bloomberg model
//! has been the default and only pricer since the closed-form
//! Black-on-spreads engine was decommissioned upstream.
//! The internal bloomberg_quadrature module implements the integration,
//! the internal pricer module exposes the bare pricing primitives (`npv`,
//! `theta`, `implied_vol`, the synthetic underlying CDS builder), and
//! Greek metrics (Δ, Γ, vega) are computed via the metrics module.
//!
//! # Greeks at a glance
//!
//! - **Δ** — closed-form Black-76 `N(d₁)` on the displayed ATM forward
//!   spread (matches the Bloomberg CDSO screen).
//! - **Γ** — central difference of Δ across a ±5 bp move in the
//!   displayed ATM forward.
//! - **Vega(1%)** — one-sided forward difference of the canonical
//!   quadrature NPV on a `+0.01` lognormal-vol bump.
//! - **Θ** — DOCS 2055833 §2.5 verbatim — shorten the exercise time
//!   `t_e` by `1/365.25` and re-price.
//! - **CS01 / DV01 / Spread DV01** — par-quote curve bumps via the
//!   shared sensitivities framework (re-bootstrap and re-price).
//!
//! # References
//!
//! - Bloomberg L.P. Quantitative Analytics. *Pricing Credit Index Options.*
//!   DOCS 2055833 ⟨GO⟩, March 2012.
//! - Bloomberg L.P. Quantitative Analytics. *The Bloomberg CDS Model.*
//!   DOCS 2057273 ⟨GO⟩, August 2024.
//! - O'Kane, D. (2008). *Modelling Single-name and Multi-name Credit
//!   Derivatives*. Wiley Finance, ch. 11 — background on lognormal-spread
//!   models.
//!
//! # See also
//!
//! - [`CDSOption`] for the instrument struct
//! - [`crate::instruments::credit_derivatives::cds`] for underlying CDS pricing

#[doc(hidden)]
pub mod bloomberg_quadrature;
pub(crate) mod metrics;
pub(crate) mod parameters;
#[doc(hidden)]
pub mod pricer;
mod types;

pub use parameters::CDSOptionParams;
pub use types::{CDSOption, ProtectionStartConvention};
