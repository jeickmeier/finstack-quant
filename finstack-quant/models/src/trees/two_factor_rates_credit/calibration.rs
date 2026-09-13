//! Calibration of the two-factor lattice: Ho-Lee theta fitting, hazard floors,
//! correlation feasibility and variance-retention diagnostics.

use super::*;

impl RatesCreditTree {
    /// Scan one calibrated factor's lattice for conditional-variance loss.
    ///
    /// Visits exactly the `(step, node)` pairs the backward induction visits
    /// (`step in 0..steps`, matching [`Self::scan_correlation_feasibility`] and
    /// `price_with_node_coupons`), evaluating the same per-node marginal
    /// probability pricing uses. Without mean reversion every `p` is exactly ½
    /// and retention is uniformly `1.0`, so the scan is skipped.
    pub(super) fn scan_variance_retention(
        levels: &[FactorRow],
        steps: usize,
        reference: f64,
        kappa: f64,
        sigma: f64,
        dt: f64,
    ) -> VarianceRetention {
        if kappa <= 0.0 || levels.is_empty() {
            return VarianceRetention::default();
        }
        let mut out = VarianceRetention {
            min_retention: 1.0,
            worst_step: 0,
            clamped_nodes: 0,
            total_nodes: 0,
        };
        for (step, row) in levels.iter().enumerate().take(steps) {
            for x in row.values() {
                let p = Self::mean_reverting_up_prob(x, reference, kappa, sigma, dt);
                let retention = 4.0 * p * (1.0 - p);
                out.total_nodes += 1;
                if p <= 0.0 || p >= 1.0 {
                    out.clamped_nodes += 1;
                }
                if retention < out.min_retention {
                    out.min_retention = retention;
                    out.worst_step = step;
                }
            }
        }
        out
    }

    /// Node hazard actually used by calibration and pricing.
    ///
    /// The additive-normal factor can go negative on a wide lattice; a
    /// negative hazard is not a credit state. Both the forward Arrow-Debreu
    /// recursion and the backward induction pass every node hazard through
    /// this same transform, which is what makes them exact duals and lets the
    /// tree reproduce the survival curve as the valuator sees it.
    #[inline]
    pub(super) fn effective_hazard(raw: f64) -> f64 {
        raw.max(0.0)
    }

    /// Validate explicit conditional calibration targets and return `(dt, T)`.
    pub(super) fn validate_calibration_targets(
        &self,
        targets: &RatesCreditCalibrationTargets,
    ) -> Result<(f64, f64)> {
        let steps = self.config.steps;
        if steps == 0 {
            return Err(Error::Validation(
                "rates-credit tree calibration requires at least one step".to_string(),
            ));
        }
        let expected_len = steps + 1;
        for (name, len) in [
            ("times", targets.times.len()),
            ("discount_factors", targets.discount_factors.len()),
            (
                "survival_probabilities",
                targets.survival_probabilities.len(),
            ),
        ] {
            if len != expected_len {
                return Err(Error::Validation(format!(
                    "rates-credit calibration {name} must contain steps + 1 = \
                     {expected_len} values, got {len}"
                )));
            }
        }

        let origin = targets.times[0];
        if !origin.is_finite() || origin.abs() > 1e-12 {
            return Err(Error::Validation(format!(
                "rates-credit calibration times must start at 0.0, got {origin}"
            )));
        }
        let dt = targets.times[1] - origin;
        if !dt.is_finite() || dt <= 0.0 {
            return Err(Error::Validation(format!(
                "rates-credit calibration requires a positive finite first time step, got {dt}"
            )));
        }
        let spacing_tolerance = 1e-12_f64.max(dt.abs() * 1e-10);
        for (step, pair) in targets.times.windows(2).enumerate() {
            let actual_dt = pair[1] - pair[0];
            if !pair[1].is_finite()
                || actual_dt <= 0.0
                || (actual_dt - dt).abs() > spacing_tolerance
            {
                return Err(Error::Validation(format!(
                    "rates-credit calibration requires an evenly spaced, strictly increasing \
                     time grid; interval {step} has width {actual_dt}, expected {dt}"
                )));
            }
        }

        for (name, values) in [
            ("discount_factors", targets.discount_factors.as_slice()),
            (
                "survival_probabilities",
                targets.survival_probabilities.as_slice(),
            ),
        ] {
            if (values[0] - 1.0).abs() > 1e-12 {
                return Err(Error::Validation(format!(
                    "conditional {name} must start at 1.0, got {}",
                    values[0]
                )));
            }
            if let Some((index, value)) = values
                .iter()
                .copied()
                .enumerate()
                .find(|(_, value)| !value.is_finite() || *value <= 0.0)
            {
                return Err(Error::Validation(format!(
                    "rates-credit calibration {name}[{index}] must be positive and finite, \
                     got {value}"
                )));
            }
        }
        for (step, pair) in targets.survival_probabilities.windows(2).enumerate() {
            if pair[1] > pair[0] + 1e-12 || pair[1] > 1.0 + 1e-12 {
                return Err(Error::Validation(format!(
                    "conditional survival probabilities must be non-increasing and at most \
                     1.0; interval {step} moves from {} to {}",
                    pair[0], pair[1]
                )));
            }
        }
        if !targets.recovery_rate.is_finite() || !(0.0..=1.0).contains(&targets.recovery_rate) {
            return Err(Error::Validation(format!(
                "rates-credit recovery_rate must be finite and in [0, 1], got {}",
                targets.recovery_rate
            )));
        }

        for (name, value) in [
            ("rate_vol", self.config.rate_vol),
            ("hazard_vol", self.config.hazard_vol),
            ("rate_mean_reversion", self.config.rate_mean_reversion),
            ("hazard_mean_reversion", self.config.hazard_mean_reversion),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(Error::Validation(format!(
                    "rates-credit {name} must be non-negative and finite, got {value}"
                )));
            }
        }
        if !self.config.correlation.is_finite() || !(-1.0..=1.0).contains(&self.config.correlation)
        {
            return Err(Error::Validation(format!(
                "rates-credit correlation must be finite and in [-1, 1], got {}",
                self.config.correlation
            )));
        }

        for (name, kappa) in [
            ("rate_mean_reversion", self.config.rate_mean_reversion),
            ("hazard_mean_reversion", self.config.hazard_mean_reversion),
        ] {
            if kappa > KAPPA_MAX {
                return Err(Error::Validation(format!(
                    "{name} = {kappa:.4} exceeds the binomial-lattice limit \
                     (KAPPA_MAX = {KAPPA_MAX}). At this speed the conditional variance of \
                     the factor collapses to a fraction of its intended value, which \
                     degrades option-value accuracy for callable bonds and term loans. \
                     Use HullWhiteTree for mean reversion above this threshold."
                )));
            }
        }

        Ok((dt, targets.times[steps]))
    }

    /// Calibrate both factors to explicit conditional targets using
    /// Arrow-Debreu forward induction.
    ///
    /// The rate factor matches `discount_factors`, and the hazard factor
    /// matches `survival_probabilities`. Calibration commits atomically: when
    /// validation or fitting fails, any earlier successful calibration on this
    /// instance remains unchanged.
    ///
    /// # Arguments
    ///
    /// * `targets` - evenly spaced coordinates and conditional discount,
    ///   survival, and recovery inputs measured from the valuation origin
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when the grid, targets, recovery, or model
    /// configuration is invalid, or when the requested correlation is not
    /// attainable on the calibrated lattice.
    pub fn calibrate(&mut self, targets: &RatesCreditCalibrationTargets) -> Result<()> {
        let (dt, _) = self.validate_calibration_targets(targets)?;
        let steps = self.config.steps;

        // A failed recalibration must not leave mixed old/new factors.
        let mut candidate = Self::new(self.config.clone());
        candidate.recovery_rate = targets.recovery_rate;
        candidate.calibration_times = targets.times.clone();

        candidate.rate_ref = Self::initial_instantaneous(targets.discount_factors[1], dt);
        candidate.hazard_ref = Self::initial_instantaneous(targets.survival_probabilities[1], dt);

        let rate_vol = candidate.config.rate_vol;
        let rate_kappa = candidate.config.rate_mean_reversion;
        let rate_ref = candidate.rate_ref;
        let (calibrated_rates, _) = candidate.calibrate_factor_ho_lee(
            FactorGrid {
                steps,
                dt,
                sigma: rate_vol,
                floor_at_zero: false,
            },
            &targets.discount_factors,
            |r| Self::mean_reverting_up_prob(r, rate_ref, rate_kappa, rate_vol, dt),
        )?;
        candidate.calibrated_rates = calibrated_rates;

        let hazard_vol = candidate.config.hazard_vol;
        let hazard_kappa = candidate.config.hazard_mean_reversion;
        let hazard_ref = candidate.hazard_ref;
        let (calibrated_hazards, saturation) = candidate.calibrate_factor_ho_lee(
            FactorGrid {
                steps,
                dt,
                sigma: hazard_vol,
                floor_at_zero: true,
            },
            &targets.survival_probabilities,
            |h| Self::mean_reverting_up_prob(h, hazard_ref, hazard_kappa, hazard_vol, dt),
        )?;
        candidate.calibrated_hazards = calibrated_hazards;
        candidate.hazard_floor_saturation = saturation;

        candidate.rate_variance_retention = Self::scan_variance_retention(
            &candidate.calibrated_rates,
            steps,
            candidate.rate_ref,
            rate_kappa,
            rate_vol,
            dt,
        );
        candidate.hazard_variance_retention = Self::scan_variance_retention(
            &candidate.calibrated_hazards,
            steps,
            candidate.hazard_ref,
            hazard_kappa,
            hazard_vol,
            dt,
        );
        candidate.validate_correlation_feasibility(dt)?;

        *self = candidate;
        Ok(())
    }

    /// Verify the configured correlation is attainable at every node pair.
    ///
    /// Two Bernoulli marginals admit a correlation only inside their Fréchet
    /// bounds. With both mean reversions zero every marginal is exactly ½ and
    /// the whole range `[-1, 1]` is attainable, so the scan is skipped. Mean
    /// reversion skews the corner nodes' marginals away from ½ and shrinks the
    /// attainable interval sharply: with both speeds at [`KAPPA_MAX`] over a
    /// five-year lattice the feasible `|ρ|` measures about `0.12`
    /// (σ_r = 0.012, σ_λ = 0.05, 40 steps). The exact bound depends on the
    /// volatilities and step count as well — the skew scales like
    /// `κ·(x − x_ref)·√Δt / (2σ)` — so callers should read it from
    /// [`Self::max_feasible_correlation`] rather than assume a fixed number.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] naming the offending step and node pair,
    /// their marginal probabilities, and the largest `|ρ|` feasible across the
    /// whole lattice.
    pub(super) fn validate_correlation_feasibility(&self, dt: f64) -> Result<()> {
        let rho = self.config.correlation;
        if rho == 0.0 {
            return Ok(());
        }
        // Without mean reversion every marginal is ½ and any |ρ| ≤ 1 is
        // attainable, so there is nothing to scan.
        if self.config.rate_mean_reversion == 0.0 && self.config.hazard_mean_reversion == 0.0 {
            return Ok(());
        }

        let (lattice_lo, lattice_hi, worst) = self.scan_correlation_feasibility(dt, Some(rho));
        let Some((step, i, j, p_r, p_h, node_lo, node_hi)) = worst else {
            return Ok(());
        };
        let max_feasible_abs = lattice_lo.abs().min(lattice_hi.abs()).max(0.0);
        Err(Error::Validation(format!(
            "rate_credit_correlation = {rho:.4} is not attainable on this lattice. \
             At step {step}, rate node {i} / hazard node {j}, the marginal \
             up-probabilities are p_r = {p_r:.4} and p_h = {p_h:.4}, whose Fréchet \
             bounds admit only ρ ∈ [{node_lo:.4}, {node_hi:.4}]. Across the whole \
             lattice the attainable interval is [{lattice_lo:.4}, {lattice_hi:.4}], \
             so the largest usable |ρ| is {max_feasible_abs:.4}. Mean reversion is \
             what narrows this: it skews the corner nodes' marginals away from ½. \
             Reduce |ρ|, or reduce the mean-reversion speeds \
             (rate κ = {kappa_r}, hazard κ = {kappa_h}).",
            kappa_r = self.config.rate_mean_reversion,
            kappa_h = self.config.hazard_mean_reversion,
        )))
    }

    /// Non-degenerate minimum and maximum marginal up-probabilities in a row.
    ///
    /// Degenerate marginals carry no correlation and therefore impose no
    /// Fréchet constraint. The returned indices preserve a useful diagnostic
    /// node when an extrema pair rejects the requested correlation.
    pub(super) fn marginal_probability_extrema(
        row: FactorRow,
        reference: f64,
        kappa: f64,
        sigma: f64,
        dt: f64,
    ) -> Option<((usize, f64), (usize, f64))> {
        let mut minimum: Option<(usize, f64)> = None;
        let mut maximum: Option<(usize, f64)> = None;
        for (index, level) in row.values().enumerate() {
            let probability = Self::mean_reverting_up_prob(level, reference, kappa, sigma, dt);
            if probability <= 0.0 || probability >= 1.0 {
                continue;
            }
            if minimum.is_none_or(|(_, current)| probability < current) {
                minimum = Some((index, probability));
            }
            if maximum.is_none_or(|(_, current)| probability > current) {
                maximum = Some((index, probability));
            }
        }
        minimum.zip(maximum)
    }

    /// Lattice-wide Fréchet-admissible correlation interval.
    ///
    /// For Bernoulli marginals, the upper correlation bound decreases with
    /// the absolute distance between their logits, so a cross-extrema pair is
    /// binding. The lower bound is least negative at either the joint minima
    /// or the joint maxima. Checking all four combinations of the two
    /// marginal extrema is therefore exactly equivalent to the Cartesian
    /// node-pair scan, while reducing a `steps`-row scan from cubic to
    /// quadratic work.
    ///
    /// Returns the intersection over those binding pairs together with one
    /// pair (if any) at which `probe` falls outside the admissible interval.
    #[allow(clippy::type_complexity)]
    pub(super) fn scan_correlation_feasibility(
        &self,
        dt: f64,
        probe: Option<f64>,
    ) -> (f64, f64, Option<(usize, usize, usize, f64, f64, f64, f64)>) {
        let mut lattice_lo = -1.0_f64;
        let mut lattice_hi = 1.0_f64;
        let mut worst = None;

        for step in 0..self.config.steps {
            let Some((rate_min, rate_max)) = Self::marginal_probability_extrema(
                self.calibrated_rates[step],
                self.rate_ref,
                self.config.rate_mean_reversion,
                self.config.rate_vol,
                dt,
            ) else {
                continue;
            };
            let Some((hazard_min, hazard_max)) = Self::marginal_probability_extrema(
                self.calibrated_hazards[step],
                self.hazard_ref,
                self.config.hazard_mean_reversion,
                self.config.hazard_vol,
                dt,
            ) else {
                continue;
            };

            for (i, p_r) in [rate_min, rate_max] {
                for (j, p_h) in [hazard_min, hazard_max] {
                    let Some((node_lo, node_hi)) = Self::node_correlation_range(p_r, p_h) else {
                        continue;
                    };
                    lattice_lo = lattice_lo.max(node_lo);
                    lattice_hi = lattice_hi.min(node_hi);
                    if probe.is_some_and(|rho| rho < node_lo || rho > node_hi) {
                        worst.get_or_insert((step, i, j, p_r, p_h, node_lo, node_hi));
                    }
                }
            }
        }
        (lattice_lo, lattice_hi, worst)
    }

    /// Fréchet-admissible correlation interval for two Bernoulli marginals.
    ///
    /// With `±1` coding the joint law is pinned by the single joint-up
    /// probability `a = P(up, up)`, which must satisfy
    /// `max(0, p_r + p_h − 1) ≤ a ≤ min(p_r, p_h)`, and the realized
    /// correlation is `ρ = (a − p_r·p_h) / √(var_r·var_h)`. Mapping the bounds
    /// on `a` through that relation gives the attainable `ρ` interval.
    ///
    /// Returns `None` when either marginal is degenerate (`p = 0` or `1`):
    /// that factor does not move at the node, so no correlation is
    /// expressible and the joint is the independent product.
    #[inline]
    pub(super) fn node_correlation_range(p_r: f64, p_h: f64) -> Option<(f64, f64)> {
        let var_r = p_r * (1.0 - p_r);
        let var_h = p_h * (1.0 - p_h);
        let denom = (var_r * var_h).sqrt();
        if denom.is_nan() || denom <= 0.0 {
            return None;
        }
        let a_lo = (p_r + p_h - 1.0).max(0.0);
        let a_hi = p_r.min(p_h);
        Some(((a_lo - p_r * p_h) / denom, (a_hi - p_r * p_h) / denom))
    }

    /// Initial instantaneous factor implied by the first target interval:
    /// `−ln(target(Δt))/Δt`.
    pub(super) fn initial_instantaneous(target_dt: f64, dt: f64) -> f64 {
        -target_dt.ln() / dt
    }

    /// Moment-matched up-probability for a mean-reverting factor on the additive
    /// normal lattice.
    ///
    /// # Units
    ///
    /// Every quantity below is in **absolute rate units** (rate per year), so
    /// the formula is dimensionally coherent:
    /// - `x`, `x_ref`: rate level (per year)
    /// - drift `μ = −κ·(x − x_ref)`: rate per year
    /// - lattice up/down step `σ√Δt`: rate (per year, integrated over `√Δt`)
    ///
    /// The standard moment-matched binomial up-probability for a drift `μ` on a
    /// lattice with step `±σ√Δt` is
    ///
    /// ```text
    /// p_up = ½ + (μ·Δt) / (2·σ√Δt) = ½ + μ·√Δt / (2σ)
    /// ```
    ///
    /// which matches the conditional **mean** `E[Δx] = μ·Δt`. With `κ = 0`
    /// this reduces to `p_up = ½` (plain Ho-Lee).
    ///
    /// # Mean vs variance trade-off
    ///
    /// An additive binomial lattice has a single free parameter (`p`). This
    /// function uses it to match the conditional mean, leaving the conditional
    /// variance `σ²Δt · 4p(1−p)` understated once `p ≠ ½`. Discount-curve
    /// repricing is **exact for any κ** because calibration and pricing apply
    /// the **identical** clamped probability, so the forward Arrow-Debreu
    /// recursion and backward induction remain exact duals. However,
    /// **option-value accuracy degrades as κ grows** — a limitation that
    /// matters when pricing callable bonds and term loans. For κ beyond
    /// [`KAPPA_MAX`] the degradation becomes material; use [`HullWhiteTree`]
    /// instead.
    ///
    /// [`HullWhiteTree`]: super::hull_white_tree::HullWhiteTree
    #[inline]
    pub(super) fn mean_reverting_up_prob(
        x: f64,
        x_ref: f64,
        kappa: f64,
        sigma: f64,
        dt: f64,
    ) -> f64 {
        if kappa <= 0.0 || dt <= 0.0 {
            return 0.5;
        }
        let sigma = sigma.max(1e-12);
        // Drift in absolute rate units (rate / year).
        let drift = -kappa * (x - x_ref);
        (0.5 + drift * dt.sqrt() / (2.0 * sigma)).clamp(0.0, 1.0)
    }

    /// Ho-Lee style calibration for a single factor.
    ///
    /// Builds a 1D binomial lattice with additive normal volatility (`sigma * sqrt(dt)`)
    /// and solves for a theta (drift) at each step so that the lattice-implied
    /// "discount factor" matches a target curve.
    ///
    /// - For the rate factor: conditional discount factors on the grid
    /// - For the hazard factor: conditional survival probabilities on the grid
    ///
    /// Both share the same mathematical structure: the product `exp(-x * dt)` over
    /// path nodes must match the target curve value at each maturity.
    ///
    /// `up_prob_fn` returns the up-transition probability for a node given its
    /// (post-theta) rate. It **must** be the identical function the pricing pass
    /// uses for backward induction: the forward Arrow-Debreu recursion below and
    /// the backward induction in `price()` are exact duals only when they share
    /// the same per-node probability, which is what makes the calibrated tree
    /// reprice the input curve when mean reversion is active.
    ///
    /// # Non-negative factors
    ///
    /// When `floor_at_zero` is set (the hazard factor), every node value is
    /// passed through [`Self::effective_hazard`] in **both** the state-price
    /// recursion and the theta solve, matching what `price()` applies. The
    /// floor makes the step equation non-linear in theta — `exp(-θΔt)` no
    /// longer factors out — so theta is bracketed and bisected instead of
    /// being read off in closed form. The rate factor is left unfloored:
    /// negative short rates are a legitimate market state, and the discounting
    /// in `price()` deliberately uses the raw calibrated rate.
    pub(super) fn calibrate_factor_ho_lee(
        &self,
        grid: FactorGrid,
        targets: &[f64],
        up_prob_fn: impl Fn(f64) -> f64,
    ) -> Result<(Vec<FactorRow>, HazardFloorSaturation)> {
        let FactorGrid {
            steps,
            dt,
            sigma,
            floor_at_zero,
        } = grid;
        let mut rates = Vec::with_capacity(steps + 1);
        let sqrt_dt = dt.sqrt();
        let one_move = sigma * sqrt_dt;
        let node_shift = 2.0 * one_move;

        // Initial rate: r0 = -ln(target(dt)) / dt.
        let r0 = Self::initial_instantaneous(targets[1], dt);
        let initial = if floor_at_zero {
            Self::effective_hazard(r0)
        } else {
            r0
        };
        rates.push(FactorRow {
            base: initial,
            shift: node_shift,
            nodes: 1,
        });

        let mut state_prices = vec![1.0];
        let mut saturation = HazardFloorSaturation::default();

        let effective = |x: f64| -> f64 {
            if floor_at_zero {
                Self::effective_hazard(x)
            } else {
                x
            }
        };

        for step in 0..steps {
            let next_nodes = step + 2;
            let mut next_state_prices = vec![0.0; next_nodes];
            let current_row = rates[step];
            let next_rates_base = FactorRow {
                base: current_row.value_unchecked(0) - one_move,
                shift: node_shift,
                nodes: next_nodes,
            };

            // The transition probability uses the SAME mean-reversion-aware
            // function the pricing pass applies, evaluated on the (calibrated)
            // current-row rate. Forward induction here is the exact dual of the
            // backward induction in `price()` because both use this probability.
            for (i, current_rate) in current_row.values().enumerate() {
                let q = state_prices[i];
                let df_i = (-effective(current_rate) * dt).exp();
                let p_up = up_prob_fn(current_rate);

                if i + 1 < next_nodes {
                    next_state_prices[i + 1] += q * df_i * p_up;
                }

                if i < next_nodes {
                    next_state_prices[i] += q * df_i * (1.0 - p_up);
                }
            }

            // Solve for theta so the lattice reproduces the target curve at
            // the next maturity.
            let next_target_index = step + 2;
            let theta = if next_target_index <= steps {
                let p_target = targets[next_target_index];
                if !floor_at_zero {
                    // Unfloored: exp(-θΔt) factors out, so theta is exact.
                    let mut p_model_base = 0.0;
                    for (j, &q_next) in next_state_prices.iter().enumerate() {
                        p_model_base += q_next * (-next_rates_base.value_unchecked(j) * dt).exp();
                    }
                    if p_model_base > 0.0 && p_target > 0.0 {
                        -(p_target / p_model_base).ln() / dt
                    } else {
                        0.0
                    }
                } else {
                    Self::solve_floored_theta(
                        &next_state_prices,
                        next_rates_base,
                        dt,
                        p_target,
                        step + 1,
                        &mut saturation,
                    )
                }
            } else {
                0.0
            };

            let next_rates = FactorRow {
                base: next_rates_base.base + theta,
                ..next_rates_base
            };
            if floor_at_zero {
                Self::record_floor_saturation(
                    next_rates,
                    &next_state_prices,
                    step + 1,
                    &mut saturation,
                );
            }
            rates.push(next_rates);
            state_prices = next_state_prices;
        }

        Ok((rates, saturation))
    }

    /// Solve the per-step hazard shift `θ` against the target survival
    /// probability under the non-negative transform.
    ///
    /// Finds `θ` with
    ///
    /// ```text
    /// Σ_j Q[j] · exp(−max(0, base_j + θ)·Δt) = target
    /// ```
    ///
    /// The left side is continuous and non-increasing in `θ`: it equals the
    /// total state-price mass `Σ_j Q[j]` once `θ` is low enough to floor the
    /// whole row, and tends to `0` as `θ` grows. A root therefore exists
    /// exactly when `0 < target < Σ_j Q[j]`. When the target exceeds the
    /// floored ceiling the survival curve is unreachable at this step: the
    /// shift is driven to the all-floored limit and the step is counted in
    /// `saturation.unreachable_steps` rather than failing, so a caller can
    /// still price while seeing that the configured hazard volatility is too
    /// large for the hazard level.
    pub(super) fn solve_floored_theta(
        state_prices: &[f64],
        base: FactorRow,
        dt: f64,
        target: f64,
        step: usize,
        saturation: &mut HazardFloorSaturation,
    ) -> f64 {
        let survival_at = |theta: f64| -> f64 {
            state_prices
                .iter()
                .zip(base.values())
                .map(|(q, b)| q * (-Self::effective_hazard(b + theta) * dt).exp())
                .sum::<f64>()
        };

        let total_mass: f64 = state_prices.iter().sum();
        if target.is_nan() || target <= 0.0 || total_mass <= 0.0 {
            return 0.0;
        }
        // All-floored limit: θ low enough that *every* node hits zero hazard,
        // which is the highest survival the row can produce. That requires
        // θ ≤ −max_j(base_j) — the largest base node is the last one to floor.
        let max_base = base.values().fold(f64::NEG_INFINITY, f64::max);
        let theta_all_floored = -max_base;
        if target >= total_mass {
            // Unreachable: even zero hazard everywhere survives less than the
            // target asks (the target itself already exceeds the mass carried
            // into this step).
            saturation.unreachable_steps += 1;
            saturation.worst_step = step;
            saturation.max_mass_at_floor = 1.0_f64.max(saturation.max_mass_at_floor);
            return theta_all_floored;
        }

        // Expand upward from the all-floored point until survival drops below
        // the target, then solve in that bracket. Survival is monotone
        // non-increasing in θ, so the bracket holds exactly one root.
        let lo = theta_all_floored;
        let mut hi = theta_all_floored.max(0.0) + 1.0;
        let mut guard = 0;
        while survival_at(hi) > target && guard < 200 {
            hi = theta_all_floored + (hi - theta_all_floored) * 2.0;
            guard += 1;
        }
        BrentSolver::new()
            .solve_in_bracket(|theta| survival_at(theta) - target, lo, hi)
            .unwrap_or(0.5 * (lo + hi))
    }

    /// Record how much state-price mass sits on the zero hazard floor at a
    /// calibrated step.
    pub(super) fn record_floor_saturation(
        rates: FactorRow,
        state_prices: &[f64],
        step: usize,
        saturation: &mut HazardFloorSaturation,
    ) {
        let total: f64 = state_prices.iter().sum();
        if total <= 0.0 {
            return;
        }
        let floored: f64 = rates
            .values()
            .zip(state_prices.iter())
            .filter(|(rate, _)| *rate <= 0.0)
            .map(|(_, q)| *q)
            .sum();
        let share = (floored / total).clamp(0.0, 1.0);
        if share > saturation.max_mass_at_floor {
            saturation.max_mass_at_floor = share;
            saturation.worst_step = step;
        }
    }
}
