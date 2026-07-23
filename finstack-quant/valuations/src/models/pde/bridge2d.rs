//! Feynman-Kac bridge for the Heston stochastic volatility model.
//!
//! Converts Heston model parameters into a [`PdeProblem2D`] for pricing via
//! ADI finite differences. Works in log-spot (`x = ln S`) and variance
//! (`y = v`) coordinates:
//!
//! ```text
//! du/dt = 0.5 v d²u/dx² + 0.5 σ_v² v d²u/dy² + ρ σ_v v d²u/(dx dy)
//!       + (r - q - 0.5 v) du/dx + κ(θ - v) du/dy - r u
//! ```
//!
//! # Boundary Conditions
//!
//! - **x-lower** (deep OTM): Dirichlet(0) for calls, Linear for puts
//! - **x-upper** (deep ITM): Linear for calls, Dirichlet(0) for puts
//! - **v-lower** (v → 0): reduces to 1D PDE `du/dt = (r-q)·du/dx + κθ·du/dv - r·u`;
//!   handled via Linear extrapolation
//! - **v-upper** (v → ∞): Linear extrapolation (option value becomes insensitive)
//!
//! # References
//!
//! - Heston, S. L. (1993). "A Closed-Form Solution for Options with Stochastic
//!   Volatility." *Review of Financial Studies*, 6(2), 327-343.
//! - In 't Hout, K. J. & Foulon, S. (2010). "ADI finite difference schemes for
//!   option pricing in the Heston model with correlation." *Int. J. of Numerical
//!   Analysis and Modeling*, 7(2).

use super::boundary::BoundaryCondition;
use super::problem2d::PdeProblem2D;

/// Heston PDE in log-spot / variance coordinates.
///
/// # Fields
///
/// All parameters follow the conventions in
/// [`crate::models::closed_form::heston::HestonParams`].
pub struct HestonPde {
    /// Risk-free interest rate (continuous, decimal).
    pub r: f64,
    /// Continuous dividend yield (decimal).
    pub q: f64,
    /// Mean reversion speed of variance.
    pub kappa: f64,
    /// Long-run variance level (θ).
    pub theta_v: f64,
    /// Volatility of variance (σ_v).
    pub sigma_v: f64,
    /// Correlation between spot and variance (-1 < ρ < 1).
    pub rho: f64,
    /// Strike price.
    pub strike: f64,
    /// True for call, false for put.
    pub is_call: bool,
}

impl PdeProblem2D for HestonPde {
    fn diffusion_xx(&self, _x: f64, y: f64, _t: f64) -> f64 {
        // 0.5 * v
        0.5 * y.max(0.0)
    }

    fn diffusion_yy(&self, _x: f64, y: f64, _t: f64) -> f64 {
        // 0.5 * σ_v² * v
        0.5 * self.sigma_v * self.sigma_v * y.max(0.0)
    }

    fn mixed_diffusion(&self, _x: f64, y: f64, _t: f64) -> f64 {
        // ρ * σ_v * v
        self.rho * self.sigma_v * y.max(0.0)
    }

    fn convection_x(&self, _x: f64, y: f64, _t: f64) -> f64 {
        // r - q - 0.5 * v
        self.r - self.q - 0.5 * y.max(0.0)
    }

    fn convection_y(&self, _x: f64, y: f64, _t: f64) -> f64 {
        // κ(θ - v)
        self.kappa * (self.theta_v - y)
    }

    fn reaction(&self, _x: f64, _y: f64, _t: f64) -> f64 {
        -self.r
    }

    fn terminal_condition(&self, x: f64, _y: f64) -> f64 {
        let s = x.exp();
        if self.is_call {
            (s - self.strike).max(0.0)
        } else {
            (self.strike - s).max(0.0)
        }
    }

    fn boundary_x_lower(&self, _y: f64, _t: f64) -> BoundaryCondition {
        if self.is_call {
            BoundaryCondition::Dirichlet(0.0) // Deep OTM call
        } else {
            BoundaryCondition::Linear // Deep ITM put
        }
    }

    fn boundary_x_upper(&self, _y: f64, _t: f64) -> BoundaryCondition {
        if self.is_call {
            BoundaryCondition::Linear // Deep ITM call
        } else {
            BoundaryCondition::Dirichlet(0.0) // Deep OTM put
        }
    }

    fn boundary_y_lower(&self, _x: f64, _t: f64) -> BoundaryCondition {
        // At v = 0 the PDE degenerates. Linear extrapolation is robust.
        BoundaryCondition::Linear
    }

    fn boundary_y_upper(&self, _x: f64, _t: f64) -> BoundaryCondition {
        // At very high variance, option value is insensitive → linear extrapolation.
        BoundaryCondition::Linear
    }
}

#[cfg(test)]
mod tests {
    use super::super::adi::{fill_boundaries, CraigSneydStepper};
    use super::super::grid::Grid1D;
    use super::super::grid2d::Grid2D;
    use super::super::solver2d::Solver2D;
    use super::*;

    /// Heston Fourier reference price for validation.
    ///
    /// Uses the existing analytical implementation for comparison.
    #[allow(clippy::too_many_arguments)]
    fn heston_call_reference(
        spot: f64,
        strike: f64,
        maturity: f64,
        r: f64,
        q: f64,
        kappa: f64,
        theta: f64,
        sigma_v: f64,
        rho: f64,
        v0: f64,
    ) -> f64 {
        use crate::models::closed_form::heston::{heston_call_price_fourier, HestonParams};
        let params =
            HestonParams::new(r, q, kappa, theta, sigma_v, rho, v0).expect("valid heston params");
        heston_call_price_fourier(spot, strike, maturity, &params)
    }

    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn heston_pde_vs_fourier_atm() {
        let spot: f64 = 100.0;
        let strike = 100.0;
        let maturity = 1.0;
        let r = 0.05;
        let q = 0.02;
        let kappa = 2.0;
        let theta_v = 0.04; // 20% vol long-run
        let sigma_v = 0.3;
        let rho = -0.7;
        let v0 = 0.04;

        let exact = heston_call_reference(
            spot, strike, maturity, r, q, kappa, theta_v, sigma_v, rho, v0,
        );

        let pde = HestonPde {
            r,
            q,
            kappa,
            theta_v,
            sigma_v,
            rho,
            strike,
            is_call: true,
        };

        // Grid: log-spot concentrated near ln(strike), variance from 0 to 1.0
        let x_min = (spot * 0.05).ln();
        let x_max = (spot * 10.0).ln();
        let v_min = 0.001;
        let v_max = 1.5;

        let gx =
            Grid1D::sinh_concentrated(x_min, x_max, 201, spot.ln(), 0.1).expect("valid x-grid");
        let gy = Grid1D::sinh_concentrated(v_min, v_max, 81, theta_v, 0.15).expect("valid v-grid");
        let grid = Grid2D::new(gx, gy);

        let solver = Solver2D::builder()
            .grid(grid)
            .craig_sneyd_rannacher(4, 400)
            .build()
            .expect("valid solver");

        let solution = solver
            .solve(&pde, maturity)
            .expect("Heston ATM grid is within the MCS stability regime");
        let computed = solution.interpolate(spot.ln(), v0);

        let rel_error = (computed - exact).abs() / exact;
        assert!(
            rel_error < 0.02,
            "Heston PDE vs Fourier: computed={computed:.6}, exact={exact:.6}, rel_err={rel_error:.4e}"
        );
    }

    /// Fast, default-running Fourier anchor.
    ///
    /// The full-resolution Fourier convergence tests are `#[ignore]`d as
    /// slow, and the always-running put-call parity test is insensitive to
    /// the mixed-derivative (correlation) term and the variance dynamics —
    /// `C − P` satisfies a driftless linear PDE regardless. Without this
    /// anchor, a wrong-signed `ρσ_v·v·S` cross term would pass the default
    /// suite. The OTM strike is the discriminating one: at K=120 with
    /// ρ=−0.7 the smile skew moves the price by far more than the coarse
    /// tolerance, while at ATM the ρ-sensitivity is smallest.
    #[test]
    fn heston_pde_vs_fourier_coarse_anchor() {
        let spot: f64 = 100.0;
        let maturity = 1.0;
        let r = 0.05;
        let q = 0.02;
        let kappa = 2.0;
        let theta_v = 0.04;
        let sigma_v = 0.3;
        let rho = -0.7;
        let v0 = 0.04;

        let x_min = (spot * 0.05).ln();
        let x_max = (spot * 10.0).ln();
        let v_min = 0.001;
        let v_max = 1.5;

        for strike in [100.0, 120.0] {
            let exact = heston_call_reference(
                spot, strike, maturity, r, q, kappa, theta_v, sigma_v, rho, v0,
            );

            let pde = HestonPde {
                r,
                q,
                kappa,
                theta_v,
                sigma_v,
                rho,
                strike,
                is_call: true,
            };

            let gx = Grid1D::sinh_concentrated(x_min, x_max, 141, strike.ln(), 0.1)
                .expect("valid x-grid");
            let gy =
                Grid1D::sinh_concentrated(v_min, v_max, 61, theta_v, 0.15).expect("valid v-grid");
            let grid = Grid2D::new(gx, gy);

            let solver = Solver2D::builder()
                .grid(grid)
                .craig_sneyd_rannacher(4, 150)
                .build()
                .expect("valid solver");

            let solution = solver
                .solve(&pde, maturity)
                .expect("Heston coarse grid is within the MCS stability regime");
            let computed = solution.interpolate(spot.ln(), v0);

            let rel_error = (computed - exact).abs() / exact;
            assert!(
                rel_error < 0.025,
                "Heston PDE coarse anchor K={strike}: computed={computed:.6}, \
                 exact={exact:.6}, rel_err={rel_error:.4e}"
            );
        }
    }

    /// A strong-mean-reversion Heston configuration (κ=10) drives the cell
    /// Péclet number in the variance direction well above the old
    /// `MCS_PECLET_MAX = 4` ceiling near the `v`-floor. The solver must
    /// handle it via the per-node upwind switch (first-order, monotone in
    /// convection-dominated cells) rather than rejecting the solve outright
    /// — and the price must still track the Fourier reference.
    #[test]
    fn heston_pde_high_kappa_solves_via_upwinding() {
        let spot: f64 = 100.0;
        let strike = 100.0;
        let maturity = 1.0;
        let r = 0.05;
        let q = 0.02;
        let kappa = 10.0;
        let theta_v = 0.04;
        let sigma_v = 0.3;
        let rho = -0.7;
        let v0 = 0.04;

        let exact = heston_call_reference(
            spot, strike, maturity, r, q, kappa, theta_v, sigma_v, rho, v0,
        );

        let pde = HestonPde {
            r,
            q,
            kappa,
            theta_v,
            sigma_v,
            rho,
            strike,
            is_call: true,
        };

        let x_min = (spot * 0.05).ln();
        let x_max = (spot * 10.0).ln();
        let gx =
            Grid1D::sinh_concentrated(x_min, x_max, 141, strike.ln(), 0.1).expect("valid x-grid");
        let gy = Grid1D::sinh_concentrated(0.001, 1.5, 61, theta_v, 0.15).expect("valid v-grid");
        let grid = Grid2D::new(gx, gy);

        let solver = Solver2D::builder()
            .grid(grid)
            .craig_sneyd_rannacher(4, 150)
            .build()
            .expect("valid solver");

        let solution = solver
            .solve(&pde, maturity)
            .expect("high-kappa Heston must solve via upwinding, not PecletViolation");
        let computed = solution.interpolate(spot.ln(), v0);

        let rel_error = (computed - exact).abs() / exact;
        assert!(
            rel_error < 0.025,
            "high-kappa Heston vs Fourier: computed={computed:.6}, exact={exact:.6}, \
             rel_err={rel_error:.4e}"
        );
    }

    #[test]
    fn heston_pde_put_call_parity() {
        let spot: f64 = 100.0;
        let strike = 105.0;
        let maturity = 0.5;
        let r = 0.03;
        let q = 0.01;
        let kappa = 1.5;
        let theta_v = 0.04;
        let sigma_v = 0.4;
        let rho = -0.5;
        let v0 = 0.06;

        let x_min = (spot * 0.1).ln();
        let x_max = (spot * 5.0).ln();
        let v_min = 0.001;
        let v_max = 1.0;

        let gx =
            Grid1D::sinh_concentrated(x_min, x_max, 121, spot.ln(), 0.1).expect("valid x-grid");
        let gy = Grid1D::sinh_concentrated(v_min, v_max, 51, theta_v, 0.2).expect("valid v-grid");

        // Solve call
        let pde_call = HestonPde {
            r,
            q,
            kappa,
            theta_v,
            sigma_v,
            rho,
            strike,
            is_call: true,
        };
        let grid_call = Grid2D::new(gx.clone(), gy.clone());
        let solver_call = Solver2D::builder()
            .grid(grid_call)
            .craig_sneyd(200)
            .build()
            .expect("valid");
        let sol_call = solver_call
            .solve(&pde_call, maturity)
            .expect("Heston call grid is within the MCS stability regime");
        let call_price = sol_call.interpolate(spot.ln(), v0);

        // Solve put
        let pde_put = HestonPde {
            r,
            q,
            kappa,
            theta_v,
            sigma_v,
            rho,
            strike,
            is_call: false,
        };
        let grid_put = Grid2D::new(gx, gy);
        let solver_put = Solver2D::builder()
            .grid(grid_put)
            .craig_sneyd(200)
            .build()
            .expect("valid");
        let sol_put = solver_put
            .solve(&pde_put, maturity)
            .expect("Heston put grid is within the MCS stability regime");
        let put_price = sol_put.interpolate(spot.ln(), v0);

        // Put-call parity: C - P = S*exp(-qT) - K*exp(-rT)
        let forward_diff = spot * (-q * maturity).exp() - strike * (-r * maturity).exp();
        let parity_error = (call_price - put_price - forward_diff).abs();
        let scale = call_price.max(put_price).max(1.0);
        let rel_parity = parity_error / scale;

        assert!(
            rel_parity < 0.02,
            "Put-call parity: C={call_price:.4}, P={put_price:.4}, diff={forward_diff:.4}, error={parity_error:.6e}"
        );
    }

    /// Solve the Heston PDE for a call with a caller-supplied ADI stepper and
    /// return the interpolated price at `(ln spot, v0)`.
    ///
    /// Mirrors [`Solver2D::solve`] but takes the stepper directly so a test
    /// can drive either the full MCS scheme or the bare Douglas scheme over an
    /// identical grid.
    fn solve_heston_call(
        stepper: &CraigSneydStepper,
        pde: &HestonPde,
        grid: &Grid2D,
        spot: f64,
        v0: f64,
        maturity: f64,
    ) -> f64 {
        let nx = grid.nx();
        let ny = grid.ny();
        let nx_int = grid.nx_interior();
        let ny_int = grid.ny_interior();

        let mut u_full = vec![0.0; nx * ny];
        for i in 0..nx {
            for j in 0..ny {
                u_full[i * ny + j] =
                    pde.terminal_condition(grid.x().points()[i], grid.y().points()[j]);
            }
        }
        let mut u_int = vec![0.0; nx_int * ny_int];
        for ii in 0..nx_int {
            for jj in 0..ny_int {
                u_int[ii * ny_int + jj] = u_full[(ii + 1) * ny + (jj + 1)];
            }
        }

        let levels = stepper.time_levels(maturity);
        for step in 0..stepper.n_steps() {
            stepper
                .step(
                    pde,
                    grid,
                    &mut u_full,
                    &mut u_int,
                    levels[step],
                    levels[step + 1],
                    step,
                )
                .expect("Heston test grid is within the MCS stability regime");
        }
        fill_boundaries(pde, grid, &mut u_full, &u_int, 0.0);
        grid.interpolate(&u_full, spot.ln(), v0)
    }

    /// Modified Craig-Sneyd ADI at strong negative correlation (rho = -0.9),
    /// validated against the independent Heston Fourier closed form.
    ///
    /// What the MCS corrector buys, stated accurately. The corrector-less
    /// Douglas scheme (predictor + two implicit unidirectional sweeps) is
    /// unconditionally stable only for `theta >= 1/2`; at `theta = 1/3` it is
    /// an *inadmissible* scheme — unstable even for pure diffusion, with or
    /// without a mixed-derivative term. The production stepper runs MCS at
    /// `theta = 1/3`, where the MCS corrector stages are exactly what make
    /// `theta = 1/3` admissible and second-order accurate. The corrector does
    /// not "stabilize the mixed term": it lowers the admissible `theta` bound.
    ///
    /// This test makes three checks on one shared grid (so differences isolate
    /// the time-stepping scheme):
    ///
    /// 1. **Correctness anchor.** MCS (`theta = 1/3`, with corrector) matches
    ///    the Heston Fourier reference to a tight tolerance at rho = -0.9.
    /// 2. **Admissibility.** Bare Douglas at `theta = 1/3` (no corrector)
    ///    diverges by orders of magnitude — demonstrating that `theta = 1/3`
    ///    *requires* the MCS corrector to be a valid scheme.
    /// 3. **Honest benefit.** Bare Douglas at `theta = 1/2` is a legitimate,
    ///    stable scheme; MCS `theta = 1/3` achieves comparable-or-better
    ///    accuracy against the Fourier reference. Comparing against a valid
    ///    Douglas baseline isolates the order/admissibility gain rather than a
    ///    spurious stability cliff.
    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn heston_pde_mcs_vs_douglas_high_correlation() {
        let spot: f64 = 100.0;
        let strike = 100.0;
        let maturity = 1.0;
        let r = 0.03;
        let q = 0.0;
        let kappa = 1.5;
        let theta_v = 0.04;
        let sigma_v = 0.3;
        let rho = -0.9; // strong negative correlation
        let v0 = 0.04;

        let exact = heston_call_reference(
            spot, strike, maturity, r, q, kappa, theta_v, sigma_v, rho, v0,
        );

        let pde = HestonPde {
            r,
            q,
            kappa,
            theta_v,
            sigma_v,
            rho,
            strike,
            is_call: true,
        };

        // One grid shared by all schemes so the comparison isolates the
        // time-stepping scheme.
        let x_min = (spot * 0.05).ln();
        let x_max = (spot * 10.0).ln();
        let v_min = 0.001;
        let v_max = 1.5;
        let gx =
            Grid1D::sinh_concentrated(x_min, x_max, 201, spot.ln(), 0.1).expect("valid x-grid");
        let gy = Grid1D::sinh_concentrated(v_min, v_max, 81, theta_v, 0.15).expect("valid v-grid");

        let n_steps = 200;

        // Full Modified Craig-Sneyd scheme (theta = 1/3) with Rannacher
        // smoothing.
        let mcs_price = solve_heston_call(
            &CraigSneydStepper::with_rannacher(4, n_steps),
            &pde,
            &Grid2D::new(gx.clone(), gy.clone()),
            spot,
            v0,
            maturity,
        );

        // Bare Douglas at theta = 1/3 (no MCS corrector): inadmissible scheme.
        let douglas_theta13 = solve_heston_call(
            &CraigSneydStepper::douglas_for_test(1.0 / 3.0, n_steps),
            &pde,
            &Grid2D::new(gx.clone(), gy.clone()),
            spot,
            v0,
            maturity,
        );

        // Bare Douglas at theta = 1/2 (no MCS corrector): a legitimate,
        // unconditionally stable scheme — used as an accuracy baseline.
        let douglas_theta12 = solve_heston_call(
            &CraigSneydStepper::douglas_for_test(0.5, n_steps),
            &pde,
            &Grid2D::new(gx, gy),
            spot,
            v0,
            maturity,
        );

        let mcs_err = (mcs_price - exact).abs() / exact;
        let douglas13_err = (douglas_theta13 - exact).abs() / exact;
        let douglas12_err = (douglas_theta12 - exact).abs() / exact;

        // (1) Correctness anchor: MCS matches the Fourier reference tightly
        // even at rho = -0.9.
        assert!(
            mcs_err < 1e-3,
            "MCS vs Fourier at rho=-0.9: mcs={mcs_price:.6}, exact={exact:.6}, rel_err={mcs_err:.4e}"
        );

        // (2) Admissibility: bare Douglas at theta = 1/3 is an inadmissible
        // scheme (the corrector-less scheme is unconditionally stable only for
        // theta >= 1/2). It diverges far outside any plausible price band
        // (non-finite or wildly large). This demonstrates that theta = 1/3
        // *requires* the MCS corrector — not that the mixed term destabilizes
        // Douglas (the corrector-less scheme is unstable at theta = 1/3 even
        // for pure diffusion).
        let douglas13_inadmissible = !douglas_theta13.is_finite() || douglas13_err > 1.0;
        assert!(
            douglas13_inadmissible,
            "expected bare Douglas at theta=1/3 to be inadmissible (unstable without \
             the MCS corrector): douglas={douglas_theta13:.6}, exact={exact:.6}, \
             rel_err={douglas13_err:.4e}"
        );

        // (3) Honest benefit: bare Douglas at theta = 1/2 is a legitimate,
        // stable scheme and is itself reasonably accurate here. MCS at
        // theta = 1/3 (second-order) matches the Fourier reference at least as
        // well as the valid Douglas baseline does — isolating the
        // order/admissibility gain rather than a spurious stability cliff.
        assert!(
            douglas_theta12.is_finite() && douglas12_err < 0.02,
            "expected bare Douglas at theta=1/2 to be stable and reasonably \
             accurate: douglas={douglas_theta12:.6}, exact={exact:.6}, \
             rel_err={douglas12_err:.4e}"
        );
        assert!(
            mcs_err <= douglas12_err + 1e-9,
            "expected MCS (theta=1/3, second-order) to match the Fourier reference \
             at least as well as the valid Douglas theta=1/2 baseline: \
             mcs_err={mcs_err:.4e}, douglas_theta12_err={douglas12_err:.4e}"
        );
    }

    /// Modified Craig-Sneyd ADI at strong *positive* correlation (rho = +0.9),
    /// validated against the Heston Fourier closed form.
    ///
    /// Companion to [`heston_pde_mcs_vs_douglas_high_correlation`], which uses
    /// rho = -0.9. The mixed-derivative term `rho * sigma_v * v` flips sign
    /// with rho, so the cross-derivative stencil contributes with the opposite
    /// sign here. Pinning MCS against the independent Fourier reference at
    /// rho = +0.9 is cheap insurance against a sign-of-rho cross-stencil bug
    /// that a single-sign test would miss.
    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn heston_pde_mcs_positive_correlation() {
        let spot: f64 = 100.0;
        let strike = 100.0;
        let maturity = 1.0;
        let r = 0.03;
        let q = 0.0;
        let kappa = 1.5;
        let theta_v = 0.04;
        let sigma_v = 0.3;
        let rho = 0.9; // strong positive correlation
        let v0 = 0.04;

        let exact = heston_call_reference(
            spot, strike, maturity, r, q, kappa, theta_v, sigma_v, rho, v0,
        );

        let pde = HestonPde {
            r,
            q,
            kappa,
            theta_v,
            sigma_v,
            rho,
            strike,
            is_call: true,
        };

        let x_min = (spot * 0.05).ln();
        let x_max = (spot * 10.0).ln();
        let v_min = 0.001;
        let v_max = 1.5;
        let gx =
            Grid1D::sinh_concentrated(x_min, x_max, 201, spot.ln(), 0.1).expect("valid x-grid");
        let gy = Grid1D::sinh_concentrated(v_min, v_max, 81, theta_v, 0.15).expect("valid v-grid");

        let mcs_price = solve_heston_call(
            &CraigSneydStepper::with_rannacher(4, 200),
            &pde,
            &Grid2D::new(gx, gy),
            spot,
            v0,
            maturity,
        );

        let mcs_err = (mcs_price - exact).abs() / exact;
        assert!(
            mcs_err < 1e-3,
            "MCS vs Fourier at rho=+0.9: mcs={mcs_price:.6}, exact={exact:.6}, rel_err={mcs_err:.4e}"
        );
    }
}
