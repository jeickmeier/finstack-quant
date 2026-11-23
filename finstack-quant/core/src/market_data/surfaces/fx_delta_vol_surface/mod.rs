//! FX delta-based volatility surface and checked delta/quote math.
//!
//! Stores FX option volatility quotes in delta space (ATM DNS, 25-delta RR/BF,
//! optional 10-delta RR/BF) alongside explicit pair, quote profile, and optional
//! expiry clock metadata. Delta-to-strike conversion, volatility evaluation,
//! and strike-grid materialization live in `finstack-quant-models`.
//!
//! # Delta Convention
//!
//! This module uses **forward delta** (premium-unadjusted) throughout:
//!
//! ```text
//! Delta_call = N(d1)
//! d1 = [ln(F/K) + 0.5 * sigma^2 * T] / (sigma * sqrt(T))
//! ```
//!
//! Inverting gives:
//!
//! ```text
//! K_call(p) = F * exp(-N_inv(p) * sigma_call * sqrt(T) + 0.5 * sigma_call^2 * T)
//! K_put(p)  = F * exp(-N_inv(1 - p) * sigma_put * sqrt(T) + 0.5 * sigma_put^2 * T)
//! K_DNS     = F * exp(0.5 * sigma_ATM^2 * T)
//! ```
//!
//! # Smile Butterfly Convention
//!
//! Risk reversals and butterflies use the two-volatility arithmetic **smile
//! butterfly** convention:
//!
//! ```text
//! RR_p = sigma_call,p - sigma_put,p
//! BF_p = 0.5 * (sigma_call,p + sigma_put,p) - sigma_ATM
//! sigma_call,p = sigma_ATM + BF_p + 0.5 * RR_p
//! sigma_put,p  = sigma_ATM + BF_p - 0.5 * RR_p
//! ```
//!
//! # References
//!
//! - Wystup (2006), *FX Options and Structured Products*, Ch. 1 `docs/REFERENCES.md#wystup-fx-options`
//! - Clark (2011), *Foreign Exchange Option Pricing*, Ch. 3-4 `docs/REFERENCES.md#clark-fx-options`

pub(super) mod conventions;
pub(super) mod delta_math;
mod expansion;
pub(super) mod surface;
#[cfg(test)]
mod tests;
