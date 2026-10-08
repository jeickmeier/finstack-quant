//! Feynman-Kac bridge: converts pricing parameters into PDE problems.
//!
//! Provides ready-made [`PdeProblem1D`] implementations for common pricing
//! setups ([`BlackScholesPde`], [`LocalVolPde`]) so that pricers don't need to
//! implement the trait from scratch.

use super::boundary::BoundaryCondition;
use super::problem::PdeProblem1D;
use crate::volatility::local_vol::LocalVolSurface;

/// Vanilla payoff at log-spot `x`.
fn vanilla_payoff(x: f64, strike: f64, is_call: bool) -> f64 {
    let s = x.exp(); // x = ln(S)
    if is_call {
        (s - strike).max(0.0)
    } else {
        (strike - s).max(0.0)
    }
}

/// Far-field condition of a vanilla option at the lower (`upper = false`) or
/// upper edge of the log-spot grid: zero on the deep out-of-the-money side,
/// affine in `S = exp(x)` on the other.
fn vanilla_boundary(is_call: bool, upper: bool) -> BoundaryCondition {
    if is_call == upper {
        BoundaryCondition::LinearInExp
    } else {
        BoundaryCondition::Dirichlet(0.0)
    }
}

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
        vanilla_payoff(x, self.strike, self.is_call)
    }

    fn lower_boundary(&self, _t: f64) -> BoundaryCondition {
        vanilla_boundary(self.is_call, false)
    }

    fn upper_boundary(&self, _t: f64) -> BoundaryCondition {
        vanilla_boundary(self.is_call, true)
    }

    fn is_time_homogeneous(&self) -> bool {
        true
    }
}

/// Dupire local-volatility PDE in log-spot coordinates.
///
/// The Black-Scholes PDE with the constant volatility replaced by a local
/// volatility `σ_loc(t, S)` read from a [`LocalVolSurface`]:
///
/// ```text
/// du/dt = 0.5σ_loc² d²u/dx² + (r - q - 0.5σ_loc²) du/dx - r u,   σ_loc = σ_loc(t, eˣ)
/// ```
///
/// where `x = ln(S)`. The solver passes each coefficient the calendar time
/// `t` of the time level being assembled (it steps from `t = maturity` down
/// to `t = 0`), which is the time axis of the surface: no conversion to
/// time-to-maturity is involved. With the surface extracted by
/// [`LocalVolSurface::from_implied_vol`] under forwards `S₀·exp((r − q)·T)`,
/// the solution at `x = ln(S₀)` reprices the European options of the implied
/// surface (Dupire 1994).
///
/// Terminal and boundary conditions are those of [`BlackScholesPde`]. The
/// coefficients depend on time, so the operator is reassembled at every step.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_models::pde::{Grid1D, LocalVolPde, Solver1D};
/// use finstack_quant_models::volatility::local_vol::LocalVolSurface;
///
/// // 25% volatility below 100 and 20% above, constant in time.
/// let surface = LocalVolSurface::new(vec![1.0], vec![99.0, 101.0], vec![0.25, 0.20])?;
/// let problem = LocalVolPde {
///     surface,
///     rate: 0.03,
///     dividend: 0.01,
///     strike: 100.0,
///     is_call: true,
/// };
/// let spot: f64 = 100.0;
/// let grid = Grid1D::sinh_concentrated((0.2 * spot).ln(), (5.0 * spot).ln(), 201, spot.ln(), 0.1)?;
/// let solution = Solver1D::builder()
///     .grid(grid)
///     .rannacher(4, 100)
///     .build()?
///     .solve(&problem, 1.0)?;
/// let price = solution.interpolate(spot.ln());
/// assert!(price > 8.0 && price < 11.0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # References
///
/// - Dupire, B. (1994). "Pricing with a Smile." *Risk*, 7(1), 18-20.
///   `docs/REFERENCES.md#dupire-1994`
/// - Gatheral, J. (2006). *The Volatility Surface: A Practitioner's Guide*.
///   Wiley. Chapter 1. `docs/REFERENCES.md#gatheral-volatility-surface`
pub struct LocalVolPde {
    /// Local volatility `σ_loc(t, S)` as an annualized decimal: expiry axis
    /// in years from the valuation date, strike axis in spot price units.
    /// Flat outside its grid.
    pub surface: LocalVolSurface,
    /// Risk-free rate (continuous, decimal).
    pub rate: f64,
    /// Continuous dividend yield (decimal).
    pub dividend: f64,
    /// Strike price.
    pub strike: f64,
    /// True for call, false for put.
    pub is_call: bool,
}

impl LocalVolPde {
    /// Local variance `σ_loc(t, eˣ)²` at log-spot `x` and calendar time `t`.
    fn local_variance(&self, x: f64, t: f64) -> f64 {
        let sigma = self.surface.value(t, x.exp());
        sigma * sigma
    }
}

impl PdeProblem1D for LocalVolPde {
    fn diffusion(&self, x: f64, t: f64) -> f64 {
        0.5 * self.local_variance(x, t)
    }

    fn convection(&self, x: f64, t: f64) -> f64 {
        self.rate - self.dividend - 0.5 * self.local_variance(x, t)
    }

    fn reaction(&self, _x: f64, _t: f64) -> f64 {
        -self.rate
    }

    fn terminal_condition(&self, x: f64) -> f64 {
        vanilla_payoff(x, self.strike, self.is_call)
    }

    fn lower_boundary(&self, _t: f64) -> BoundaryCondition {
        vanilla_boundary(self.is_call, false)
    }

    fn upper_boundary(&self, _t: f64) -> BoundaryCondition {
        vanilla_boundary(self.is_call, true)
    }

    fn is_time_homogeneous(&self) -> bool {
        false
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

    // -----------------------------------------------------------------------
    // Local volatility
    // -----------------------------------------------------------------------

    use crate::monte_carlo::process::local_vol::LocalVolParams;
    use crate::monte_carlo::simulate::{
        simulate_paths, PathSimulationSpec, ProcessSpec, SchemeSpec, TimeGridSpec,
    };
    use crate::volatility::implied_vol_black;
    use crate::volatility::local_vol::test_support::{forwards, linspace, SKEWED};

    const LV_SPOT: f64 = 100.0;
    const LV_RATE: f64 = 0.03;
    const LV_DIVIDEND: f64 = 0.01;

    /// Solve a vanilla problem at `LV_SPOT` on a log-spot grid concentrated at
    /// the strike, with Rannacher start-up.
    fn solve_at_spot(problem: &dyn PdeProblem1D, strike: f64, expiry: f64) -> f64 {
        let grid = Grid1D::sinh_concentrated(
            (0.1 * LV_SPOT).ln(),
            (8.0 * LV_SPOT).ln(),
            801,
            strike.ln(),
            0.1,
        )
        .expect("valid grid");
        let steps = (400.0 * expiry).round() as usize;
        Solver1D::builder()
            .grid(grid)
            .rannacher(4, steps)
            .build()
            .expect("valid solver")
            .solve(problem, expiry)
            .expect("local-vol solve")
            .interpolate(LV_SPOT.ln())
    }

    /// Local volatility of the shared skewed SSVI surface: 80 expiries from
    /// 0.02 to 1.6 years by 361 strikes from 40 to 220.
    fn skewed_local_vol() -> LocalVolSurface {
        let expiries = linspace(0.02, 1.6, 80);
        let strikes = linspace(40.0, 220.0, 361);
        let forwards = forwards(LV_SPOT, LV_RATE - LV_DIVIDEND, &expiries);
        let implied = SKEWED.surface(&expiries, &strikes, &forwards);
        LocalVolSurface::from_implied_vol(&implied, &forwards).expect("surface extracts")
    }

    /// Black implied volatility of a discounted option price.
    fn implied_vol(price: f64, strike: f64, expiry: f64, is_call: bool) -> f64 {
        let forward = LV_SPOT * ((LV_RATE - LV_DIVIDEND) * expiry).exp();
        let undiscounted = price * (LV_RATE * expiry).exp();
        implied_vol_black(undiscounted, forward, strike, expiry, is_call).expect("price inverts")
    }

    /// With a flat surface the coefficients equal the Black-Scholes ones at
    /// every node and time, so the two problems give the same solution; only
    /// the per-step reassembly differs.
    #[test]
    fn local_vol_pde_with_a_flat_surface_is_the_black_scholes_pde() {
        let sigma = 0.2;
        let flat = LocalVolSurface::new(vec![0.5, 1.0], vec![50.0, 150.0], vec![sigma; 4])
            .expect("flat grid");
        for (strike, is_call) in [(90.0, true), (100.0, true), (115.0, false)] {
            let local = LocalVolPde {
                surface: flat.clone(),
                rate: LV_RATE,
                dividend: LV_DIVIDEND,
                strike,
                is_call,
            };
            let black_scholes = BlackScholesPde {
                sigma,
                rate: LV_RATE,
                dividend: LV_DIVIDEND,
                strike,
                maturity: 1.0,
                is_call,
            };
            assert!(!local.is_time_homogeneous());
            let (a, b) = (
                solve_at_spot(&local, strike, 1.0),
                solve_at_spot(&black_scholes, strike, 1.0),
            );
            assert!((a - b).abs() < 1e-10 * b, "K={strike}: {a} vs {b}");
        }
    }

    /// The defining Dupire property through the PDE: prices on the extracted
    /// local volatility reprice the implied surface. Out-of-the-money options
    /// on an 801-node grid with 400 steps per year.
    ///
    /// Measured: errors from -0.4 to -4.2 basis points of implied volatility,
    /// largest at the 80 strike of the six-month expiry and shrinking like
    /// `1/T` (at the money: -1.8, -1.0 and -0.7 basis points at 0.5, 1 and
    /// 1.5 years). That is a fixed shortfall of about 4e-5 of total variance
    /// from the first 0.02 years, where the implied grid has no expiry and
    /// the local volatility is extrapolated flat from its first row.
    #[test]
    fn local_vol_pde_reprices_the_implied_surface() {
        let surface = skewed_local_vol();
        let mut worst_bp: f64 = 0.0;
        for expiry in [0.5, 1.0, 1.5] {
            let forward = LV_SPOT * ((LV_RATE - LV_DIVIDEND) * expiry).exp();
            for strike in [80.0, 90.0, 100.0, 110.0, 120.0] {
                let is_call = strike >= forward;
                let problem = LocalVolPde {
                    surface: surface.clone(),
                    rate: LV_RATE,
                    dividend: LV_DIVIDEND,
                    strike,
                    is_call,
                };
                let vol = implied_vol(
                    solve_at_spot(&problem, strike, expiry),
                    strike,
                    expiry,
                    is_call,
                );
                let target = SKEWED.implied_vol((strike / forward).ln(), expiry);
                let error_bp = (vol - target) * 1e4;
                println!("T={expiry} K={strike}: PDE {vol:.5} target {target:.5} {error_bp:+.2}bp");
                worst_bp = worst_bp.max(error_bp.abs());
                assert!(
                    error_bp.abs() < 5.0,
                    "T={expiry} K={strike}: PDE implied vol {vol} vs surface {target} \
                     ({error_bp:+.2}bp)"
                );
            }
        }
        println!("worst PDE repricing error {worst_bp:.2}bp");
    }

    /// The local volatility must be read at calendar time. A European price
    /// under a strike-independent volatility only sees the integrated
    /// variance and cannot tell the two time axes apart, so this uses the
    /// skewed surface, whose skew decays with time: reading it at
    /// time-to-maturity misprices the wing by far more than the solver error
    /// (measured: -1.1 basis points at calendar time, +192 reversed).
    #[test]
    fn local_vol_pde_reads_the_surface_at_calendar_time() {
        /// The same problem with the surface read at `maturity - t`.
        struct TimeToMaturity {
            inner: LocalVolPde,
            maturity: f64,
        }
        impl PdeProblem1D for TimeToMaturity {
            fn diffusion(&self, x: f64, t: f64) -> f64 {
                self.inner.diffusion(x, self.maturity - t)
            }
            fn convection(&self, x: f64, t: f64) -> f64 {
                self.inner.convection(x, self.maturity - t)
            }
            fn reaction(&self, x: f64, t: f64) -> f64 {
                self.inner.reaction(x, t)
            }
            fn terminal_condition(&self, x: f64) -> f64 {
                self.inner.terminal_condition(x)
            }
            fn lower_boundary(&self, t: f64) -> BoundaryCondition {
                self.inner.lower_boundary(t)
            }
            fn upper_boundary(&self, t: f64) -> BoundaryCondition {
                self.inner.upper_boundary(t)
            }
        }

        let (strike, expiry) = (80.0, 1.5);
        let forward = LV_SPOT * ((LV_RATE - LV_DIVIDEND) * expiry).exp();
        let target = SKEWED.implied_vol((strike / forward).ln(), expiry);
        let problem = || LocalVolPde {
            surface: skewed_local_vol(),
            rate: LV_RATE,
            dividend: LV_DIVIDEND,
            strike,
            is_call: false,
        };
        let calendar = implied_vol(
            solve_at_spot(&problem(), strike, expiry),
            strike,
            expiry,
            false,
        );
        let reversed = implied_vol(
            solve_at_spot(
                &TimeToMaturity {
                    inner: problem(),
                    maturity: expiry,
                },
                strike,
                expiry,
            ),
            strike,
            expiry,
            false,
        );
        println!(
            "calendar {:+.2}bp, reversed {:+.2}bp",
            (calendar - target) * 1e4,
            (reversed - target) * 1e4
        );
        assert!((calendar - target).abs() < 5e-4);
        assert!(
            (reversed - target).abs() > 30e-4,
            "a reversed time axis must be visible: calendar {calendar}, reversed {reversed}, \
             target {target}"
        );
    }

    /// PDE and Monte Carlo on the same local volatility agree within the
    /// Monte Carlo sampling error and its first-order time-stepping bias.
    /// Measured: +0.7, +0.4 and -1.5 basis points of implied volatility.
    #[test]
    fn local_vol_pde_agrees_with_monte_carlo() {
        let surface = skewed_local_vol();
        let expiry = 1.0;
        let paths = simulate_paths(&PathSimulationSpec {
            process: ProcessSpec::LocalVol(
                LocalVolParams::new(LV_RATE, LV_DIVIDEND, surface.clone()).expect("valid"),
            ),
            scheme: SchemeSpec::Default,
            initial_state: vec![LV_SPOT],
            time_grid: TimeGridSpec::Uniform {
                expiry,
                num_steps: 100,
            },
            num_paths: 100_000,
            seed: 77,
            antithetic: true,
            fbm: None,
        })
        .expect("simulation");
        let stride = paths.times.len();
        let forward = LV_SPOT * ((LV_RATE - LV_DIVIDEND) * expiry).exp();
        for strike in [85.0, 100.0, 115.0] {
            let is_call = strike >= forward;
            let mean_payoff = paths
                .values
                .chunks(stride)
                .map(|path| {
                    let spot = path[stride - 1];
                    if is_call {
                        (spot - strike).max(0.0)
                    } else {
                        (strike - spot).max(0.0)
                    }
                })
                .sum::<f64>()
                / paths.num_simulated_paths as f64;
            let monte_carlo = implied_vol_black(mean_payoff, forward, strike, expiry, is_call)
                .expect("Monte Carlo price inverts");
            let problem = LocalVolPde {
                surface: surface.clone(),
                rate: LV_RATE,
                dividend: LV_DIVIDEND,
                strike,
                is_call,
            };
            let pde = implied_vol(
                solve_at_spot(&problem, strike, expiry),
                strike,
                expiry,
                is_call,
            );
            let difference_bp = (pde - monte_carlo) * 1e4;
            println!("K={strike}: PDE {pde:.5} MC {monte_carlo:.5} {difference_bp:+.1}bp");
            assert!(
                difference_bp.abs() < 25.0,
                "K={strike}: PDE {pde} vs Monte Carlo {monte_carlo} ({difference_bp:+.1}bp)"
            );
        }
    }
}
