//! Feynman-Kac bridge: converts pricing parameters into PDE problems.
//!
//! Provides ready-made [`PdeProblem1D`] implementations for common pricing
//! setups (Black-Scholes, local vol) so that pricers don't need to implement
//! the trait from scratch.

use super::boundary::BoundaryCondition;
use super::problem::PdeProblem1D;

/// Black-Scholes PDE in log-spot coordinates.
///
/// Solves the PDE for European/American option pricing under constant
/// volatility, risk-free rate, and dividend yield:
///
/// ```text
/// du/dt = 0.5σ² d²u/dx² + (r - q - 0.5σ²) du/dx - r u
/// ```
///
/// where `x = ln(S)` is the log-spot coordinate.
///
/// # Boundary Conditions
///
/// - **Call**: zero at the deep-OTM lower edge and affine in `S = exp(x)`
///   at the upper edge.
/// - **Put**: affine in `S = exp(x)` at the lower edge and zero at the
///   deep-OTM upper edge.
///
/// The affine far field imposes vanishing spot gamma, `u_xx = u_x`, rather
/// than vanishing curvature in the log-spot coordinate.
pub struct BlackScholesPde {
    /// Volatility (annualized, decimal).
    pub sigma: f64,
    /// Risk-free rate (continuous, decimal).
    pub rate: f64,
    /// Continuous dividend yield (decimal).
    pub dividend: f64,
    /// Strike price.
    pub strike: f64,
    /// Time to maturity (for boundary conditions).
    pub maturity: f64,
    /// True for call, false for put.
    pub is_call: bool,
}

impl PdeProblem1D for BlackScholesPde {
    fn diffusion(&self, _x: f64, _t: f64) -> f64 {
        0.5 * self.sigma * self.sigma
    }

    fn convection(&self, _x: f64, _t: f64) -> f64 {
        self.rate - self.dividend - 0.5 * self.sigma * self.sigma
    }

    fn reaction(&self, _x: f64, _t: f64) -> f64 {
        -self.rate
    }

    fn terminal_condition(&self, x: f64) -> f64 {
        let s = x.exp(); // x = ln(S)
        if self.is_call {
            (s - self.strike).max(0.0)
        } else {
            (self.strike - s).max(0.0)
        }
    }

    fn lower_boundary(&self, _t: f64) -> BoundaryCondition {
        if self.is_call {
            // Deep OTM call → value ≈ 0
            BoundaryCondition::Dirichlet(0.0)
        } else {
            BoundaryCondition::LinearInExp
        }
    }

    fn upper_boundary(&self, _t: f64) -> BoundaryCondition {
        if self.is_call {
            BoundaryCondition::LinearInExp
        } else {
            // Deep OTM put → value ≈ 0
            BoundaryCondition::Dirichlet(0.0)
        }
    }

    fn is_time_homogeneous(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::super::grid::Grid1D;
    use super::super::solver::Solver1D;
    use super::*;

    /// Black-Scholes analytical price for European call (for validation).
    ///
    /// Uses `core::math::norm_cdf` rather than a local Abramowitz-Stegun
    /// approximation: this is the *reference* the PDE solver is graded
    /// against, so its own error should not eat into the test's tolerance.
    fn bs_call(s: f64, k: f64, r: f64, q: f64, sigma: f64, t: f64) -> f64 {
        use finstack_quant_core::math::norm_cdf;
        let d1 = ((s / k).ln() + (r - q + 0.5 * sigma * sigma) * t) / (sigma * t.sqrt());
        let d2 = d1 - sigma * t.sqrt();
        s * (-q * t).exp() * norm_cdf(d1) - k * (-r * t).exp() * norm_cdf(d2)
    }

    /// Black-Scholes analytical price for European put (for validation).
    fn bs_put(s: f64, k: f64, r: f64, q: f64, sigma: f64, t: f64) -> f64 {
        use finstack_quant_core::math::norm_cdf;
        let d1 = ((s / k).ln() + (r - q + 0.5 * sigma * sigma) * t) / (sigma * t.sqrt());
        let d2 = d1 - sigma * t.sqrt();
        k * (-r * t).exp() * norm_cdf(-d2) - s * (-q * t).exp() * norm_cdf(-d1)
    }

    #[test]
    fn bs_call_pde_vs_analytical() {
        let s = 100.0;
        let k = 100.0;
        let r = 0.05;
        let q = 0.02;
        let sigma = 0.2;
        let t = 1.0;

        let exact = bs_call(s, k, r, q, sigma, t);

        let pde = BlackScholesPde {
            sigma,
            rate: r,
            dividend: q,
            strike: k,
            maturity: t,
            is_call: true,
        };

        let x_min = (s * 0.01).ln(); // ~4.5 standard deviations
        let x_max = (s * 5.0).ln();
        let grid = Grid1D::sinh_concentrated(x_min, x_max, 301, s.ln(), 0.1).expect("valid grid");
        let solver = Solver1D::builder()
            .grid(grid)
            .crank_nicolson(300)
            .build()
            .expect("valid solver");

        let solution = solver
            .solve(&pde, t)
            .expect("Crank-Nicolson solve is unconditionally stable");
        let computed = solution.interpolate(s.ln());

        let rel_error = (computed - exact).abs() / exact;
        assert!(
            rel_error < 0.001,
            "BS call PDE error: computed={computed:.6}, exact={exact:.6}, rel_err={rel_error:.6e}"
        );
    }

    #[test]
    fn bs_put_pde_vs_analytical() {
        let s = 100.0;
        let k = 110.0;
        let r = 0.05;
        let q = 0.0;
        let sigma = 0.25;
        let t = 0.5;

        let exact = bs_put(s, k, r, q, sigma, t);

        let pde = BlackScholesPde {
            sigma,
            rate: r,
            dividend: q,
            strike: k,
            maturity: t,
            is_call: false,
        };

        let x_min = (s * 0.01).ln();
        let x_max = (s * 5.0).ln();
        let grid = Grid1D::sinh_concentrated(x_min, x_max, 301, s.ln(), 0.1).expect("valid grid");
        let solver = Solver1D::builder()
            .grid(grid)
            .crank_nicolson(300)
            .build()
            .expect("valid solver");

        let solution = solver
            .solve(&pde, t)
            .expect("Crank-Nicolson solve is unconditionally stable");
        let computed = solution.interpolate(s.ln());

        let rel_error = (computed - exact).abs() / exact;
        assert!(
            rel_error < 0.001,
            "BS put PDE error: computed={computed:.6}, exact={exact:.6}, rel_err={rel_error:.6e}"
        );
    }

    #[test]
    fn bs_pde_delta_reasonable() {
        let s: f64 = 100.0;
        let k = 100.0;
        let r = 0.05;
        let q = 0.0;
        let sigma = 0.2;
        let t = 1.0;

        let pde = BlackScholesPde {
            sigma,
            rate: r,
            dividend: q,
            strike: k,
            maturity: t,
            is_call: true,
        };

        let x_min = (s * 0.01).ln();
        let x_max = (s * 5.0).ln();
        let grid = Grid1D::sinh_concentrated(x_min, x_max, 301, s.ln(), 0.1).expect("valid grid");
        let solver = Solver1D::builder()
            .grid(grid)
            .crank_nicolson(300)
            .build()
            .expect("valid solver");

        let solution = solver
            .solve(&pde, t)
            .expect("Crank-Nicolson solve is unconditionally stable");

        // Delta in log-spot space: dV/dx. To get dV/dS, divide by S.
        let delta_log = solution.delta(s.ln());
        let delta_spot = delta_log / s;

        // ATM call delta should be roughly 0.5-0.7
        assert!(
            (0.3..=0.9).contains(&delta_spot),
            "ATM call delta={delta_spot:.4}, expected ~0.5-0.7"
        );
    }

    #[test]
    fn convection_dominated_calls_are_nonnegative_and_converge_under_refinement() {
        // Centered convection previously returned -0.085976 and -0.225921
        // for these two finite, positive-volatility vanilla calls.
        // The monotone fallback restores positivity but adds O(h) diffusion:
        // the production 200/100 mesh alone does not resolve these nearly
        // deterministic OTM payoffs. Require convergence as both grids refine.
        for (spot, dividend) in [(109.7_f64, 0.10_f64), (126.0, 0.25)] {
            let strike = 100.0_f64;
            let sigma = 0.001;
            let spread = 5.0 * sigma + dividend;
            let exact = bs_call(spot, strike, 0.0, dividend, sigma, 1.0);
            assert!((0.0..1e-10).contains(&exact), "near-deterministic OTM call");
            let problem = BlackScholesPde {
                sigma,
                rate: 0.0,
                dividend,
                strike,
                maturity: 1.0,
                is_call: true,
            };
            let mut errors = Vec::new();
            for (nodes, steps) in [(200, 100), (400, 200), (800, 400)] {
                let grid = Grid1D::sinh_concentrated(
                    strike.ln() - spread,
                    spot.ln() + spread,
                    nodes,
                    strike.ln(),
                    0.1,
                )
                .expect("fixed production domain");
                let solution = Solver1D::builder()
                    .grid(grid)
                    .rannacher(4, steps)
                    .build()
                    .expect("solver")
                    .solve(&problem, 1.0)
                    .expect("convection-dominated solve");
                let price = solution.interpolate(spot.ln());
                assert!(
                    price >= 0.0 && price <= spot * (-dividend).exp(),
                    "call value {price} violates bounds for S={spot}, q={dividend}, nodes={nodes}"
                );
                errors.push((price - exact).abs());
            }
            assert!(errors.windows(2).all(|pair| pair[1] < pair[0]));
            assert!(errors[2] < errors[0] / 4.0, "mesh errors {errors:?}");
            assert!(
                errors[2] < 0.001 * strike,
                "800-node error exceeds 10 basis points of strike: {errors:?}"
            );
        }
    }

    #[test]
    fn log_spot_far_field_converges_to_deep_itm_call_value() {
        // At this strike the entire upper half of the domain is effectively
        // linear in S. Linear extrapolation in ln(S) left a >1 unit bias even
        // with 1,001 nodes; refining the mesh could not correct it.
        let exact = bs_call(100.0, 1.0, 0.05, 0.0, 0.5, 1.0);
        let problem = BlackScholesPde {
            sigma: 0.5,
            rate: 0.05,
            dividend: 0.0,
            strike: 1.0,
            maturity: 1.0,
            is_call: true,
        };
        let mut errors = Vec::new();
        for nodes in [101, 401] {
            let grid = Grid1D::uniform(0.01_f64.ln(), 200.0_f64.ln(), nodes)
                .expect("fixed log-spot domain");
            let solution = Solver1D::builder()
                .grid(grid)
                .rannacher(4, 1000)
                .build()
                .expect("solver")
                .solve(&problem, 1.0)
                .expect("deep ITM solve");
            errors.push((solution.interpolate(100.0_f64.ln()) - exact).abs());
        }
        assert!(errors[1] < 0.03, "fine-grid price error {}", errors[1]);
        assert!(errors[1] < errors[0] / 4.0, "mesh errors {errors:?}");
    }
}
