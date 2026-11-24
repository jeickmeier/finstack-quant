//! Bloomberg CDSO numerical-quadrature pricer.
//!
//! Implements the model published in:
//!
//! - Bloomberg L.P. Quantitative Analytics. *Pricing Credit Index Options.*
//!   DOCS 2055833 ⟨GO⟩, March 2012.
//!   `docs/REFERENCES.md#bloomberg-cdso`
//!
//! and uses the Bloomberg CDS pricer (DOCS 2057273,
//! `docs/REFERENCES.md#bloomberg-cds-model`) for the bootstrapped
//! `F_0` calibration target.
//!
//! # Model summary (DOCS 2055833 §2.2, Eqs. 2.2–2.5)
//!
//! ```text
//! S_te    = m · exp(−½σ²t_e + σ·√t_e · ε),     ε ~ N(0,1)
//! V_te    = ξ N (S_te − c) · L_te(S_te)
//! H(K)    = ξ N (c − K) · A(K)
//! D       = ξ N₀ · loss(t_v)
//! O       = P(t_e) · E_0 [ (V_te + H(K) + D)+ ]
//! F_0     = E_0 [V_te]                          (calibration anchor)
//! ```
//!
//! - `S_te` is the (random) realised forward CDS spread at expiry.
//! - `L_te(S)` is the *flat-spread* forward risky annuity at hazard
//!   `λ = S/(1−R)` over `[t_e, t_M]` — the "credit triangle" simplification
//!   §2.5: continuous coupon, constant rate to expiry → analytic `λ(S)`.
//! - `A(K)` is the same flat-spread annuity evaluated at `S = K`.
//! - `m` is calibrated so `F_0` matches the bootstrapped clean forward swap
//!   value.
//! - `ξ = +1` for payer (call), `ξ = −1` for receiver (put).
//! - For index CDS options, expected front-end protection enters the
//!   loss-adjusted calibration anchor `F_0` exactly once. The deterministic
//!   exercise term `D` contains only already-realized losses.
//!
//! The curve anchor and random-state annuity share contractual coupon dates
//! and clean accrued premium. The anchor uses observed survival; the
//! stochastic state uses the model's flat hazard. The calibrated expectation
//! equals the forward CDS value. Future protection starts at expiry and
//! front-end protection ends there. Only knockout options condition that
//! forward value on survival to expiry.
//!
//! # Numerical integration
//!
//! The smooth calibration expectation uses a trapezoidal normal-driver
//! grid with step `Δz = 0.05`. The option payoff has an exercise kink, so
//! it uses adaptive Simpson integration with a total absolute error target
//! of `1e-12` per unit notional. Integration failures propagate to callers.
//!
//! All time inputs use **calendar days / 365** (DOCS 2055833 §2.1, matching
//! FinancePy's `bloomberg_cdso::G_DAYS_IN_YEAR = 365.0`). Premium-leg accrual factors come
//! from the synthetic underlying CDS in its native day count (Act/360 for
//! USD CDX/iTraxx Main).

mod engine;
#[cfg(test)]
mod tests;

pub use engine::{
    ForwardCdsContext, calibrate_lognormal_mean, normal_integral, price_with_calibrated_mean,
    quadrature_payoff, z_limit,
};
pub(crate) use engine::{forward_par_at_expiry_bp, npv, theta};
