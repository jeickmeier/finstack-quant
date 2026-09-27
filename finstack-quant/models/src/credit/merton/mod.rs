//! Merton structural credit model with distance-to-default and default probability.
//!
//! Implements the Merton (1974) model and its Black-Cox (1976) first-passage
//! extension for estimating firm default probability from balance-sheet data.
//!
//! # References
//!
//! - Merton, R. C. (1974). "On the Pricing of Corporate Debt: The Risk
//!   Structure of Interest Rates." *Journal of Finance*, 29(2), 449-470. `docs/REFERENCES.md#merton-1974`
//!
//! - Black, F. & Cox, J. C. (1976). "Valuing Corporate Securities: Some
//!   Effects of Bond Indenture Provisions." *Journal of Finance*, 31(2), 351-367. `docs/REFERENCES.md#black-1976`
//!
//! - Merton, R. C. (1976). "Option Pricing When Underlying Stock Returns Are
//!   Discontinuous." *Journal of Financial Economics*, 3(1-2), 125-144.
//!   Poisson mixture behind the jump-diffusion default probability. `docs/REFERENCES.md#merton-1976-jump`
//!
//! - Finger, C. C. et al. (2002). *CreditGrades Technical Document*.
//!   RiskMetrics Group. Uncertain-barrier survival approximation. `docs/REFERENCES.md#finger-2002-creditgrades`
//!
//! - Crosbie, P. & Bohn, J. (2003). *Modeling Default Risk*. Moody's KMV.
//!   Physical-measure distance to default, EDF, and the default point. `docs/REFERENCES.md#crosbie-bohn-2003-kmv`
//!
//! - O'Kane, D. (2008). *Modelling Single-name and Multi-name Credit
//!   Derivatives*. Wiley Finance. CDS premium and protection leg
//!   discretization used by [`MertonModel::cds_par_spread`]. `docs/REFERENCES.md#o-kane-2008`
//!
//! # Spread conventions
//!
//! Three distinct credit spreads are available and they are **not**
//! interchangeable:
//!
//! - [`MertonModel::implied_spread`] — continuously compounded zero-coupon
//!   bond spread with an *exogenous* recovery paid at maturity.
//! - [`MertonModel::debt_spread`] — Merton (1974) *endogenous* debt spread,
//!   where recovery is the firm's own terminal asset value.
//! - [`MertonModel::cds_par_spread`] — ISDA-style CDS par spread built from
//!   the model's survival curve, with a premium leg, accrual on default, and
//!   discounting.
//!
//! # Examples
//!
//! ```
//! use finstack_quant_models::credit::MertonModel;
//!
//! let model = MertonModel::new(100.0, 0.20, 80.0, 0.05).unwrap();
//! let dd = model.distance_to_default(1.0);
//! let pd = model.default_probability(1.0);
//! let spread = model.implied_spread(5.0, 0.40).unwrap();
//! ```

mod calibration;
mod default_probability;
mod dynamics;
mod hazard_curve;
mod model;
mod simulation;
mod spreads;

pub use dynamics::{AssetDynamics, MertonBarrierType};
pub use model::{MertonModel, RawMertonModel};
pub use simulation::SimulatedPaths;
