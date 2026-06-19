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
/// - **Call**: `u(x_min, t) = 0` (deep OTM put), `u(x_max, t) = exp(x_max) - K*exp(-r*(T-t))`
/// - **Put**: `u(x_min, t) = K*exp(-r*(T-t)) - exp(x_min)`, `u(x_max, t) = 0`
///
/// Far-field boundaries use Linear (vanishing gamma) for robustness.
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
            // Deep ITM put → use linear extrapolation for stability
            BoundaryCondition::Linear
        }
    }

    fn upper_boundary(&self, _t: f64) -> BoundaryCondition {
        if self.is_call {
            // Deep ITM call → use linear extrapolation
            BoundaryCondition::Linear
        } else {
            // Deep OTM put → value ≈ 0
            BoundaryCondition::Dirichlet(0.0)
        }
    }

    fn is_time_homogeneous(&self) -> bool {
        true
    }
}

/// Local volatility PDE in log-spot coordinates.
///
/// Like [`BlackScholesPde`] but with a spatially- and temporally-varying
/// volatility surface `σ(S, t)`. The caller provides a closure implementing
/// the Dupire local vol function.
///
/// ```text
/// du/dt = 0.5σ(e^x, t)² d²u/dx² + (r - q - 0.5σ(e^x, t)²) du/dx - r u
/// ```
pub struct LocalVolPde<F: Fn(f64, f64) -> f64> {
    /// Local vol function σ(S, t) — takes spot (not log-spot) and time.
    pub local_vol: F,
    /// Risk-free rate.
    pub rate: f64,
    /// Dividend yield.
    pub dividend: f64,
    /// Strike price.
    pub strike: f64,
    /// True for call, false for put.
    pub is_call: bool,
}

impl<F: Fn(f64, f64) -> f64 + Send + Sync> PdeProblem1D for LocalVolPde<F> {
    fn diffusion(&self, x: f64, t: f64) -> f64 {
        let s = x.exp();
        let sigma = (self.local_vol)(s, t);
        0.5 * sigma * sigma
    }

    fn convection(&self, x: f64, t: f64) -> f64 {
        let s = x.exp();
        let sigma = (self.local_vol)(s, t);
        self.rate - self.dividend - 0.5 * sigma * sigma
    }

    fn reaction(&self, _x: f64, _t: f64) -> f64 {
        -self.rate
    }

    fn terminal_condition(&self, x: f64) -> f64 {
        let s = x.exp();
        if self.is_call {
            (s - self.strike).max(0.0)
        } else {
            (self.strike - s).max(0.0)
        }
    }

    fn lower_boundary(&self, _t: f64) -> BoundaryCondition {
        if self.is_call {
            BoundaryCondition::Dirichlet(0.0)
        } else {
            BoundaryCondition::Linear
        }
    }

    fn upper_boundary(&self, _t: f64) -> BoundaryCondition {
        if self.is_call {
            BoundaryCondition::Linear
        } else {
            BoundaryCondition::Dirichlet(0.0)
        }
    }

    fn is_time_homogeneous(&self) -> bool {
        false // local vol is time-dependent
    }
}

#[cfg(test)]
mod tests {
    use super::super::grid::Grid1D;
    use super::super::solver::Solver1D;
    use super::*;

    /// Black-Scholes analytical price for European call (for validation).
    fn bs_call(s: f64, k: f64, r: f64, q: f64, sigma: f64, t: f64) -> f64 {
        use std::f64::consts::FRAC_1_SQRT_2;
        let d1 = ((s / k).ln() + (r - q + 0.5 * sigma * sigma) * t) / (sigma * t.sqrt());
        let d2 = d1 - sigma * t.sqrt();
        let nd1 = 0.5 * (1.0 + erf(d1 * FRAC_1_SQRT_2));
        let nd2 = 0.5 * (1.0 + erf(d2 * FRAC_1_SQRT_2));
        s * (-q * t).exp() * nd1 - k * (-r * t).exp() * nd2
    }

    /// Black-Scholes analytical price for European put (for validation).
    fn bs_put(s: f64, k: f64, r: f64, q: f64, sigma: f64, t: f64) -> f64 {
        use std::f64::consts::FRAC_1_SQRT_2;
        let d1 = ((s / k).ln() + (r - q + 0.5 * sigma * sigma) * t) / (sigma * t.sqrt());
        let d2 = d1 - sigma * t.sqrt();
        let nd1 = 0.5 * (1.0 + erf(-d1 * FRAC_1_SQRT_2));
        let nd2 = 0.5 * (1.0 + erf(-d2 * FRAC_1_SQRT_2));
        k * (-r * t).exp() * nd2 - s * (-q * t).exp() * nd1
    }

    /// Error function (Abramowitz & Stegun approximation).
    fn erf(x: f64) -> f64 {
        let a1 = 0.254829592;
        let a2 = -0.284496736;
        let a3 = 1.421413741;
        let a4 = -1.453152027;
        let a5 = 1.061405429;
        let p = 0.3275911;
        let sign = if x >= 0.0 { 1.0 } else { -1.0 };
        let x = x.abs();
        let t = 1.0 / (1.0 + p * x);
        let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();
        sign * y
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
    fn local_vol_flat_matches_bs() {
        // Flat local vol = constant sigma should reproduce Black-Scholes exactly
        let s = 100.0;
        let k = 100.0;
        let r = 0.05;
        let q = 0.02;
        let sigma = 0.2;
        let t = 1.0;

        let exact = bs_call(s, k, r, q, sigma, t);

        let pde = LocalVolPde {
            local_vol: |_spot: f64, _time: f64| sigma,
            rate: r,
            dividend: q,
            strike: k,
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
        let computed = solution.interpolate(s.ln());

        let rel_error = (computed - exact).abs() / exact;
        assert!(
            rel_error < 0.001,
            "LocalVol(flat) vs BS: computed={computed:.6}, exact={exact:.6}, rel_err={rel_error:.6e}"
        );
    }
}
