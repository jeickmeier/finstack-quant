//! PDE / Finite Difference infrastructure for 1D and 2D option pricing.
//!
//! Provides a complete solver pipeline:
//! ```text
//! PdeProblem1D (coefficients + boundary conditions + domain)
//!   → TridiagOperator (discretizes PDE on a Grid1D)
//!     → TimeStepper (theta scheme: explicit/implicit/CN/Rannacher)
//!       → PenaltyExercise (American/Bermudan constraint)
//!         → PdeSolution (values + interpolation + Greeks)
//! ```
//!
//! For 2D problems (e.g., Heston stochastic volatility):
//! ```text
//! PdeProblem2D (2D coefficients + cross-derivative + 4-edge boundaries)
//!   → Operators2D (directional tridiag per axis + explicit cross-derivative)
//!     → CraigSneydStepper (Modified Craig-Sneyd ADI splitting)
//!       → PdeSolution2D (bilinear interpolation + Greeks)
//! ```
//!
//! # Usage
//!
//! For standard Black-Scholes pricing, use the [`BlackScholesPde`] bridge:
//!
//! ```
//! use finstack_quant_models::pde::*;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let pde = BlackScholesPde {
//!     sigma: 0.2, rate: 0.05, dividend: 0.0,
//!     strike: 100.0, maturity: 1.0, is_call: true,
//! };
//!
//! let grid = Grid1D::sinh_concentrated(-5.0, 5.0, 200, 0.0, 0.1)?;
//! let solver = Solver1D::builder()
//!     .grid(grid)
//!     .crank_nicolson(100)
//!     .build()?;
//!
//! let solution = solver.solve(&pde, 1.0)?;
//! let price = solution.interpolate(100.0_f64.ln());
//!
//! // An at-the-money one-year call under these parameters is worth ~10.45.
//! assert!((price - 10.45).abs() < 0.5);
//! # Ok(())
//! # }
//! ```
//!
//! For custom PDEs, implement [`PdeProblem1D`] directly.
//!
//! # Design
//!
//! Lives under `instruments/common/models/` alongside `trees/`, `closed_form/`,
//! and `volatility/`. PDE solvers are numerical pricing models, tightly coupled
//! to the valuations domain, and unlikely to be reused outside it.

mod adi;
mod boundary;
mod bridge;
mod bridge2d;
mod exercise;
mod grid;
mod grid2d;
mod operator;
mod operator2d;
mod problem;
mod problem2d;
mod solver;
mod solver2d;
mod stepper;

pub use adi::CraigSneydStepper;
pub use boundary::BoundaryCondition;
pub use bridge::BlackScholesPde;
pub use bridge2d::HestonPde;
pub use exercise::{ExerciseError, ExerciseType, PenaltyExercise};
pub use grid::{Grid1D, PdeGridError};
pub use grid2d::Grid2D;
pub use operator::TridiagOperator;
pub use operator2d::{apply_cross_derivative, Operators2D};
pub use problem::PdeProblem1D;
pub use problem2d::PdeProblem2D;
pub use solver::{PdeSolution, PdeSolverError, Solver1D, Solver1DBuilder};
pub use solver2d::{PdeSolution2D, PdeSolver2DError, Solver2D};
pub use stepper::{RannacherStepper, StepperError, ThetaStepper, TimeStepper};
