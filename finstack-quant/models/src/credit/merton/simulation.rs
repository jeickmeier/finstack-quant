use finstack_quant_core::math::random::{poisson_inverse_cdf, RandomNumberGenerator};
use finstack_quant_core::{Error, Result};

use super::{AssetDynamics, MertonModel};

/// Results from Monte Carlo path simulation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SimulatedPaths {
    /// Time grid from 0 to T.
    pub times: Vec<f64>,
    /// Asset values in row-major order: `path_idx * (num_steps + 1) + time_idx`.
    pub asset_values: Vec<f64>,
    /// Number of paths simulated.
    pub num_paths: usize,
    /// Number of time steps.
    pub num_steps: usize,
}

impl SimulatedPaths {
    /// Number of stored values per path, including the initial value.
    #[must_use]
    pub fn values_per_path(&self) -> usize {
        self.num_steps + 1
    }

    /// Return one asset value by path and time-grid index.
    #[must_use]
    pub fn get(&self, path_idx: usize, time_idx: usize) -> Option<f64> {
        if path_idx >= self.num_paths || time_idx > self.num_steps {
            return None;
        }
        self.asset_values
            .get(path_idx * self.values_per_path() + time_idx)
            .copied()
    }

    /// Return the contiguous row for one path.
    #[must_use]
    pub fn path(&self, path_idx: usize) -> Option<&[f64]> {
        if path_idx >= self.num_paths {
            return None;
        }
        let start = path_idx * self.values_per_path();
        let end = start + self.values_per_path();
        self.asset_values.get(start..end)
    }

    /// Iterate over path rows.
    pub fn iter_paths(&self) -> impl Iterator<Item = &[f64]> {
        self.asset_values.chunks_exact(self.values_per_path())
    }

    /// Materialize nested path storage for callers that need the old shape.
    #[must_use]
    pub fn to_nested(&self) -> Vec<Vec<f64>> {
        self.iter_paths().map(<[f64]>::to_vec).collect()
    }
}

struct StepJumpData {
    base_count: usize,
    anti_count: usize,
    jump_normals: Vec<f64>,
}

impl MertonModel {
    /// Simulate asset value paths using Monte Carlo.
    ///
    /// Supports GBM and jump-diffusion dynamics. Optionally uses antithetic
    /// variates to reduce variance.
    ///
    /// `CreditGrades` dynamics simulate the *asset value* as plain GBM: the
    /// CreditGrades stochastic barrier only enters the analytic
    /// default-probability formulas, not the simulated paths. Callers needing
    /// pathwise CreditGrades default times must layer the barrier on top.
    ///
    /// # Discrete monitoring
    ///
    /// This returns the raw asset grid; it applies no barrier and no
    /// Brownian-bridge correction. A caller that infers first-passage default
    /// by testing `V_t <= B` at grid points alone will **understate** default,
    /// because a path can dip below the barrier and recover between two
    /// steps. The bias grows with step size and shrinks as `O(sqrt(dt))`. To
    /// recover the continuous-monitoring law, apply the Brownian-bridge
    /// crossing probability
    /// `exp(-2 * ln(V_i/B) * ln(V_{i+1}/B) / (sigma^2 * dt))` to each
    /// surviving step, as the PIK-toggle Monte Carlo bond engine does. The
    /// analytic [`default_probability`](Self::default_probability) already
    /// uses the continuous-monitoring Black-Cox law, so a naive grid test
    /// will not reproduce it.
    ///
    /// # Arguments
    ///
    /// * `num_paths` - Number of independent paths to simulate. With
    ///   `antithetic` set, each path is paired with its sign-flipped twin
    /// * `num_steps` - Number of equally spaced time steps per path; must be
    ///   at least 1, and each path stores `num_steps + 1` values
    /// * `horizon` - Time horizon T in years spanned by the grid; must be
    ///   finite and strictly positive
    /// * `rng` - Random number generator supplying the standard normal (and,
    ///   under `JumpDiffusion`, uniform) draws; determines reproducibility
    /// * `antithetic` - When true, generate each odd-indexed path from the
    ///   negated normals of its predecessor for variance reduction
    ///
    /// # Returns
    ///
    /// [`SimulatedPaths`] containing the time grid and all simulated asset paths.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `num_steps == 0` (the time grid would
    /// be degenerate with `dt = inf`) or `horizon` is not finite and
    /// positive.
    pub fn simulate_paths(
        &self,
        num_paths: usize,
        num_steps: usize,
        horizon: f64,
        rng: &mut dyn RandomNumberGenerator,
        antithetic: bool,
    ) -> Result<SimulatedPaths> {
        if num_steps == 0 {
            return Err(Error::Validation(
                "simulate_paths: num_steps must be >= 1".into(),
            ));
        }
        if !(horizon.is_finite() && horizon > 0.0) {
            return Err(Error::Validation(format!(
                "simulate_paths: horizon must be > 0, got {horizon}"
            )));
        }
        let dt = horizon / num_steps as f64;
        let sqrt_dt = dt.sqrt();

        let times: Vec<f64> = (0..=num_steps).map(|i| i as f64 * dt).collect();

        let v0 = self.asset_value;
        let sigma = self.asset_vol;
        let r = self.risk_free_rate;
        let q = self.payout_rate;

        let (drift_per_step, jump_params) = match &self.dynamics {
            AssetDynamics::GeometricBrownian | AssetDynamics::CreditGrades { .. } => {
                let drift = (r - q - 0.5 * sigma * sigma) * dt;
                (drift, None)
            }
            AssetDynamics::JumpDiffusion {
                jump_intensity,
                jump_mean,
                jump_vol,
            } => {
                // kappa = E[e^J] - 1 where J ~ N(mu_J, sigma_J^2)
                let kappa = (jump_mean + 0.5 * jump_vol * jump_vol).exp() - 1.0;
                // Compensated drift to keep E[V(T)] = V0 * e^{(r-q)T}
                let drift = (r - q - jump_intensity * kappa - 0.5 * sigma * sigma) * dt;
                (drift, Some((*jump_intensity, *jump_mean, *jump_vol)))
            }
        };

        let diffusion = sigma * sqrt_dt;

        let (n_base, gen_antithetic) = if antithetic {
            // For num_paths requested: generate ceil(num_paths/2) base paths
            // and their mirrors. Total = 2 * n_base.
            // If num_paths is odd, we generate one extra base path without mirror
            // to hit exactly num_paths.
            let n_base = num_paths.div_ceil(2);
            (n_base, true)
        } else {
            (num_paths, false)
        };

        let values_per_path = num_steps + 1;
        let mut all_paths: Vec<f64> = Vec::with_capacity(num_paths * values_per_path);
        let mut normals = vec![0.0; num_steps];

        for _ in 0..n_base {
            normals.iter_mut().for_each(|z| *z = rng.normal(0.0, 1.0));

            let jump_data: Option<Vec<StepJumpData>> = jump_params.map(|(lambda, _, _)| {
                let lambda_dt = lambda * dt;
                (0..num_steps)
                    .map(|_| {
                        let u = rng.uniform();
                        let base_count = poisson_inverse_cdf(lambda_dt, u);
                        let anti_count =
                            poisson_inverse_cdf(lambda_dt, (1.0 - u).min(1.0 - f64::EPSILON));
                        let max_count = base_count.max(anti_count);
                        let jump_normals: Vec<f64> =
                            (0..max_count).map(|_| rng.normal(0.0, 1.0)).collect();
                        StepJumpData {
                            base_count,
                            anti_count,
                            jump_normals,
                        }
                    })
                    .collect()
            });

            all_paths.push(v0);
            let mut v = v0;

            for step in 0..num_steps {
                let z = normals[step];
                v *= (drift_per_step + diffusion * z).exp();

                if let (Some(ref jd), Some((_, mu_j, sigma_j))) = (&jump_data, jump_params) {
                    let jump_step = &jd[step];
                    for &jz in jump_step.jump_normals.iter().take(jump_step.base_count) {
                        // Jump multiplier e^J with J ~ N(mu_J, sigma_J^2): mean
                        // E[e^J] = exp(mu_J + sigma_J^2/2), matching the kappa
                        // compensator above. No Ito correction belongs on the
                        // jump itself .
                        v *= (mu_j + sigma_j * jz).exp();
                    }
                }

                all_paths.push(v);
            }

            if gen_antithetic && all_paths.len() / values_per_path < num_paths {
                all_paths.push(v0);
                let mut v_anti = v0;

                for step in 0..num_steps {
                    let z = -normals[step]; // Negated normal
                    v_anti *= (drift_per_step + diffusion * z).exp();

                    if let (Some(ref jd), Some((_, mu_j, sigma_j))) = (&jump_data, jump_params) {
                        let jump_step = &jd[step];
                        for &jz in jump_step.jump_normals.iter().take(jump_step.anti_count) {
                            let jz = -jz;
                            // Same e^J law as the base path .
                            v_anti *= (mu_j + sigma_j * jz).exp();
                        }
                    }

                    all_paths.push(v_anti);
                }
            }
        }

        // Trim to exact num_paths in case antithetic generated one extra
        all_paths.truncate(num_paths * values_per_path);

        Ok(SimulatedPaths {
            times,
            asset_values: all_paths,
            num_paths,
            num_steps,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::{AssetDynamics, BarrierType, MertonModel};

    #[test]
    fn simulate_paths_rejects_degenerate_grid() {
        let m = MertonModel::new(100.0, 0.25, 80.0, 0.04).unwrap();
        let mut rng = finstack_quant_core::math::random::Pcg64Rng::new(42);
        assert!(m.simulate_paths(10, 0, 5.0, &mut rng, false).is_err());
        assert!(m.simulate_paths(10, 60, 0.0, &mut rng, false).is_err());
    }

    // Monte Carlo path simulation tests

    #[test]
    fn simulate_paths_deterministic_with_seed() {
        use finstack_quant_core::math::random::Pcg64Rng;
        let m = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("valid");
        let mut rng1 = Pcg64Rng::new(42);
        let mut rng2 = Pcg64Rng::new(42);
        let paths1 = m
            .simulate_paths(10, 60, 5.0, &mut rng1, false)
            .expect("paths");
        let paths2 = m
            .simulate_paths(10, 60, 5.0, &mut rng2, false)
            .expect("paths");
        assert_eq!(
            paths1.path(0),
            paths2.path(0),
            "Same seed should give same paths"
        );
    }

    #[test]
    fn simulate_paths_gbm_mean_converges() {
        use finstack_quant_core::math::random::Pcg64Rng;
        let m = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("valid");
        let mut rng = Pcg64Rng::new(42);
        let paths = m
            .simulate_paths(50_000, 60, 5.0, &mut rng, true)
            .expect("paths");
        let mean_terminal: f64 = paths
            .iter_paths()
            .map(|p| *p.last().expect("non-empty"))
            .sum::<f64>()
            / paths.num_paths as f64;
        let expected = 100.0 * (0.05_f64 * 5.0).exp();
        let rel_error = (mean_terminal - expected).abs() / expected;
        assert!(
            rel_error < 0.02,
            "Mean terminal should converge to E[V(T)] = V\u{2080}\u{00d7}e^(rT) = {expected}, got {mean_terminal}, rel_err={rel_error}"
        );
    }

    #[test]
    fn simulate_paths_correct_dimensions() {
        use finstack_quant_core::math::random::Pcg64Rng;
        let m = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("valid");
        let mut rng = Pcg64Rng::new(42);
        let paths = m
            .simulate_paths(100, 60, 5.0, &mut rng, false)
            .expect("paths");
        assert_eq!(paths.num_paths, 100);
        assert_eq!(paths.num_steps, 60);
        assert_eq!(paths.times.len(), 61); // includes t=0
        assert_eq!(paths.asset_values.len(), 100 * 61);
        assert_eq!(paths.values_per_path(), 61);
        assert!(
            (paths.times[0] - 0.0).abs() < 1e-10,
            "First time should be 0"
        );
        assert!(
            (paths.times[60] - 5.0).abs() < 1e-10,
            "Last time should be horizon"
        );
        assert!(
            (paths.get(0, 0).expect("path value") - 100.0).abs() < 1e-10,
            "Should start at V\u{2080}"
        );
    }

    #[test]
    fn jump_diffusion_produces_different_paths() {
        use finstack_quant_core::math::random::Pcg64Rng;
        let m_gbm = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("valid");
        let m_jd = MertonModel::new_with_dynamics(
            100.0,
            0.20,
            80.0,
            0.05,
            0.0,
            BarrierType::Terminal,
            AssetDynamics::JumpDiffusion {
                jump_intensity: 0.5,
                jump_mean: -0.05,
                jump_vol: 0.10,
            },
        )
        .expect("valid");
        let mut rng1 = Pcg64Rng::new(42);
        let mut rng2 = Pcg64Rng::new(42);
        let paths_gbm = m_gbm
            .simulate_paths(100, 60, 5.0, &mut rng1, false)
            .expect("paths");
        let paths_jd = m_jd
            .simulate_paths(100, 60, 5.0, &mut rng2, false)
            .expect("paths");
        // JD paths should differ from GBM (different drift compensation + jumps)
        let gbm_terminal: f64 = paths_gbm
            .iter_paths()
            .map(|p| *p.last().expect("non-empty"))
            .sum::<f64>();
        let jd_terminal: f64 = paths_jd
            .iter_paths()
            .map(|p| *p.last().expect("non-empty"))
            .sum::<f64>();
        assert!(
            (gbm_terminal - jd_terminal).abs() > 1.0,
            "JD should produce different terminal values"
        );
    }

    /// Jump-diffusion martingale check : the compensated
    /// drift uses kappa = E[e^J] - 1 = exp(mu_J + sigma_J^2/2) - 1, so the
    /// simulated jump multiplier must be e^J with J ~ N(mu_J, sigma_J^2)
    /// (mean exp(mu_J + sigma_J^2/2)). With that pairing,
    /// E[V(T)] = V0 * e^{(r-q)T} holds exactly.
    #[test]
    fn simulate_paths_jump_diffusion_mean_converges() {
        use finstack_quant_core::math::random::Pcg64Rng;
        let m = MertonModel::new_with_dynamics(
            100.0,
            0.20,
            80.0,
            0.05,
            0.0,
            BarrierType::Terminal,
            AssetDynamics::JumpDiffusion {
                jump_intensity: 0.5,
                jump_mean: -0.05,
                jump_vol: 0.10,
            },
        )
        .expect("valid");
        let mut rng = Pcg64Rng::new(42);
        let paths = m
            .simulate_paths(50_000, 60, 5.0, &mut rng, true)
            .expect("paths");
        let mean_terminal: f64 = paths
            .iter_paths()
            .map(|p| *p.last().expect("non-empty"))
            .sum::<f64>()
            / paths.num_paths as f64;
        let expected = 100.0 * (0.05_f64 * 5.0).exp();
        let rel_error = (mean_terminal - expected).abs() / expected;
        assert!(
            rel_error < 0.02,
            "JD mean terminal should converge to E[V(T)] = V0*e^(rT) = {expected}, \
                 got {mean_terminal}, rel_err={rel_error}"
        );
    }
}
