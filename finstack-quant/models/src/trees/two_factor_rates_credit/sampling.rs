//! Transition probabilities, path sampling and conditional discount factors.

use super::*;

impl RatesCreditTree {
    /// Joint movement probabilities from a calibrated node.
    ///
    /// # Arguments
    ///
    /// * `step` - interval start step in `0..config.steps`
    /// * `rate_node` - rate-factor node index in `0..=step`
    /// * `hazard_node` - hazard-factor node index in `0..=step`
    ///
    /// # Errors
    ///
    /// Returns an error when the tree is uncalibrated or any index is outside
    /// the calibrated lattice.
    pub fn transition_probabilities(
        &self,
        step: usize,
        rate_node: usize,
        hazard_node: usize,
    ) -> Result<RatesCreditTransition> {
        if step >= self.config.steps || rate_node > step || hazard_node > step {
            return Err(Error::Validation(format!(
                "rates-credit transition requires step < {} and node indices <= step; \
                 got step={step}, rate_node={rate_node}, hazard_node={hazard_node}",
                self.config.steps
            )));
        }
        let dt = self.calibrated_dt()?;
        let rate = self.rate_at_node(step, rate_node)?;
        let hazard = self.hazard_at_node(step, hazard_node)?;
        let p_rate_up = Self::mean_reverting_up_prob(
            rate,
            self.rate_ref,
            self.config.rate_mean_reversion,
            self.config.rate_vol,
            dt,
        );
        let p_hazard_up = Self::mean_reverting_up_prob(
            hazard,
            self.hazard_ref,
            self.config.hazard_mean_reversion,
            self.config.hazard_vol,
            dt,
        );
        let (up_up, up_down, down_up, down_down) = self.joint_probabilities(p_rate_up, p_hazard_up);
        Ok(RatesCreditTransition {
            up_up,
            up_down,
            down_up,
            down_down,
        })
    }

    /// Sample one deterministic joint factor path with a dedicated Philox
    /// substream.
    ///
    /// # Arguments
    ///
    /// * `seed` - root Philox seed shared by a reproducible simulation run
    /// * `path_index` - unique Philox substream identifier for this logical path
    /// * `antithetic` - when true, use `1 - u` for every uniform from the same
    ///   `(seed, path_index)` stream
    ///
    /// # Errors
    ///
    /// Returns an error when the tree is uncalibrated.
    pub fn sample_path(
        &self,
        seed: u64,
        path_index: u64,
        antithetic: bool,
    ) -> Result<Vec<RatesCreditPathState>> {
        let mut path = Vec::with_capacity(self.config.steps + 1);
        self.sample_path_into(seed, path_index, antithetic, &mut path)?;
        Ok(path)
    }

    /// Sample one deterministic joint factor path into caller-owned storage.
    ///
    /// Reusing `output` avoids one allocation per path in streamed or blocked
    /// simulations. Existing contents are cleared before the new path is
    /// written.
    ///
    /// # Arguments
    ///
    /// * `seed` - root Philox seed shared by a reproducible simulation run
    /// * `path_index` - unique Philox substream identifier for this logical path
    /// * `antithetic` - when true, mirror every uniform from the same Philox
    ///   substream
    /// * `output` - reusable destination receiving `config.steps + 1` states
    ///
    /// # Errors
    ///
    /// Returns an error when the tree is uncalibrated.
    pub fn sample_path_into(
        &self,
        seed: u64,
        path_index: u64,
        antithetic: bool,
        output: &mut Vec<RatesCreditPathState>,
    ) -> Result<()> {
        let dt = self.calibrated_dt()?;
        let times = self.time_grid()?;
        let mut rng = PhiloxRng::with_stream(seed, path_index);
        let mut uniform = [0.0_f64; 1];
        let mut rate_node = 0_usize;
        let mut hazard_node = 0_usize;

        output.clear();
        output.reserve(self.config.steps + 1);
        for (step, &time) in times.iter().enumerate() {
            let short_rate = self.rate_at_node(step, rate_node)?;
            let hazard_rate = Self::effective_hazard(self.hazard_at_node(step, hazard_node)?);
            let (discount_to_next, survival_to_next, default_to_next) = if step < self.config.steps
            {
                let discount = (-short_rate * dt).exp();
                let hazard_exponent = -hazard_rate * dt;
                let survival = hazard_exponent.exp();
                let default = -hazard_exponent.exp_m1();
                (discount, survival, default)
            } else {
                (1.0, 1.0, 0.0)
            };
            output.push(RatesCreditPathState {
                step,
                time,
                rate_node,
                hazard_node,
                short_rate,
                hazard_rate,
                discount_to_next,
                survival_to_next,
                default_to_next,
            });

            if step == self.config.steps {
                break;
            }
            let probabilities = self.transition_probabilities(step, rate_node, hazard_node)?;
            rng.fill_u01(&mut uniform);
            let draw = if antithetic {
                1.0 - uniform[0]
            } else {
                uniform[0]
            };
            let (rate_up, hazard_up) = Self::sampled_moves(probabilities, draw);
            rate_node += usize::from(rate_up);
            hazard_node += usize::from(hazard_up);
        }
        Ok(())
    }

    /// Resume a deterministic sampled factor path over one inclusive step range.
    ///
    /// The returned states are bit-for-bit identical to
    /// `sample_path(seed, path_index, antithetic)?[checkpoint.step..=end_step]`.
    /// This supports bounded-memory consumers that checkpoint product state at
    /// time-block boundaries and must not replay the full factor-path prefix.
    ///
    /// # Arguments
    ///
    /// * `seed` - Root Philox seed used for the original path.
    /// * `path_index` - Philox substream identifier used for the original path.
    /// * `antithetic` - Whether each uniform draw is mirrored as `1 - u`.
    /// * `checkpoint` - Factor node indices and first lattice step of the segment.
    /// * `end_step` - Last lattice step to return, inclusive.
    /// * `output` - Reusable destination cleared before the segment is written.
    ///
    /// # Errors
    ///
    /// Returns an error if the tree is uncalibrated, the checkpoint is outside
    /// the calibrated lattice, or `end_step` precedes the checkpoint or exceeds
    /// the calibrated horizon.
    pub fn sample_path_segment_into(
        &self,
        seed: u64,
        path_index: u64,
        antithetic: bool,
        checkpoint: RatesCreditPathCheckpoint,
        end_step: usize,
        output: &mut Vec<RatesCreditPathState>,
    ) -> Result<()> {
        if checkpoint.step > end_step
            || end_step > self.config.steps
            || checkpoint.rate_node > checkpoint.step
            || checkpoint.hazard_node > checkpoint.step
        {
            return Err(Error::Validation(format!(
                "rates-credit path segment requires checkpoint nodes <= step and {} <= end_step <= {}; got checkpoint=({}, {}, {}), end_step={end_step}",
                checkpoint.step,
                self.config.steps,
                checkpoint.step,
                checkpoint.rate_node,
                checkpoint.hazard_node
            )));
        }
        let dt = self.calibrated_dt()?;
        let times = self.time_grid()?;
        let draw_offset = u64::try_from(checkpoint.step).map_err(|_| {
            Error::Validation("rates-credit path checkpoint step exceeds Philox range".to_string())
        })?;
        let mut rng = PhiloxRng::with_stream_u01_offset(seed, path_index, draw_offset);
        let mut uniform = [0.0_f64; 1];
        let mut rate_node = checkpoint.rate_node;
        let mut hazard_node = checkpoint.hazard_node;

        output.clear();
        output.reserve(end_step - checkpoint.step + 1);
        for (step, &time) in times
            .iter()
            .enumerate()
            .take(end_step + 1)
            .skip(checkpoint.step)
        {
            let short_rate = self.rate_at_node(step, rate_node)?;
            let hazard_rate = Self::effective_hazard(self.hazard_at_node(step, hazard_node)?);
            let (discount_to_next, survival_to_next, default_to_next) = if step < self.config.steps
            {
                let discount = (-short_rate * dt).exp();
                let hazard_exponent = -hazard_rate * dt;
                (discount, hazard_exponent.exp(), -hazard_exponent.exp_m1())
            } else {
                (1.0, 1.0, 0.0)
            };
            output.push(RatesCreditPathState {
                step,
                time,
                rate_node,
                hazard_node,
                short_rate,
                hazard_rate,
                discount_to_next,
                survival_to_next,
                default_to_next,
            });

            if step == end_step {
                break;
            }
            let probabilities = self.transition_probabilities(step, rate_node, hazard_node)?;
            rng.fill_u01(&mut uniform);
            let draw = if antithetic {
                1.0 - uniform[0]
            } else {
                uniform[0]
            };
            let (rate_up, hazard_up) = Self::sampled_moves(probabilities, draw);
            rate_node += usize::from(rate_up);
            hazard_node += usize::from(hazard_up);
        }
        Ok(())
    }

    #[inline]
    pub(super) fn sampled_moves(probabilities: RatesCreditTransition, draw: f64) -> (bool, bool) {
        let up_up_cutoff = probabilities.up_up;
        let up_down_cutoff = up_up_cutoff + probabilities.up_down;
        let down_up_cutoff = up_down_cutoff + probabilities.down_up;
        if draw < up_up_cutoff {
            (true, true)
        } else if draw < up_down_cutoff {
            (true, false)
        } else if draw < down_up_cutoff {
            (false, true)
        } else {
            (false, false)
        }
    }

    /// Couple the two marginal up-probabilities into joint cell probabilities.
    ///
    /// The joint law of two Bernoullis has exactly one degree of freedom once
    /// the marginals are fixed, so it is built from the single joint-up
    /// probability
    ///
    /// ```text
    /// a = P(up, up) = p_r·p_h + ρ·√(var_r·var_h)
    /// ```
    ///
    /// and the remaining three cells follow by **subtraction**:
    ///
    /// ```text
    /// p_ud = p_r − a,  p_du = p_h − a,  p_dd = 1 − p_r − p_h + a
    /// ```
    ///
    /// Deriving them this way makes both marginals exact by construction —
    /// `p_uu + p_ud = p_r` and `p_uu + p_du = p_h` hold identically — which is
    /// what keeps the forward Arrow-Debreu recursion and the backward
    /// induction exact duals, so the calibrated tree still reprices both input
    /// curves. The previous formulation clamped all four cells independently
    /// and renormalised, which silently perturbed the marginals (and hence the
    /// curve repricing) whenever a cell hit the clamp.
    ///
    /// `a` is confined to the Fréchet interval
    /// `[max(0, p_r + p_h − 1), min(p_r, p_h)]` purely as a numerical guard:
    /// [`Self::validate_correlation_feasibility`] has already proved at
    /// calibration time that the configured `ρ` is attainable at every node,
    /// so the clamp is unreachable in a calibrated tree and is asserted as
    /// such in debug builds.
    #[inline]
    pub(super) fn joint_probabilities(&self, p_r: f64, p_h: f64) -> (f64, f64, f64, f64) {
        let var_r = p_r * (1.0 - p_r);
        let var_h = p_h * (1.0 - p_h);
        let a_target = p_r * p_h + self.config.correlation * (var_r * var_h).sqrt();

        let a_lo = (p_r + p_h - 1.0).max(0.0);
        let a_hi = p_r.min(p_h);
        debug_assert!(
            a_target >= a_lo - 1e-9 && a_target <= a_hi + 1e-9,
            "calibration must have rejected an infeasible correlation before \
             pricing: p_r={p_r}, p_h={p_h}, a={a_target}, bounds=[{a_lo}, {a_hi}]"
        );
        let a = a_target.clamp(a_lo, a_hi);

        // Marginals are exact by construction.
        (a, p_r - a, p_h - a, 1.0 - p_r - p_h + a)
    }

    /// Tree-conditional risk-free discount factors `P(t_n → t_m | rate node)`.
    ///
    /// For each rate node `i` at `reset_step`, returns the conditional
    /// zero-coupon price to `payment_step` implied by backward induction on
    /// the **rate-marginal** lattice using the **raw calibrated rates** —
    /// no OAS shift, no survival, no positive floor. The rate marginal is
    /// Markov on its own binomial lattice because the joint transition
    /// probabilities are built from the marginals by Fréchet coupling
    /// (`p_uu + p_ud = p_r` identically), so conditioning on the rate node
    /// alone is exact.
    ///
    /// This is the **forward-derivation operator** for node-dependent
    /// floating coupons: the simply-compounded node forward over the
    /// schedule accrual fraction τ is `(1/P − 1)/τ`. It is deliberately a
    /// *different* operator from the pricing-measure folding in
    /// [`Self::price_with_node_coupons`] (which applies OAS, survival, and
    /// the correlated joint transitions): node forwards are a property of
    /// the calibrated risk-free lattice and must not move with the OAS.
    ///
    /// # Arguments
    ///
    /// * `reset_step` - slice at which the forward is observed
    /// * `payment_step` - slice at which the notional would be repaid
    /// * `time_to_maturity` - total lattice horizon in years (defines `Δt`)
    ///
    /// # Returns
    ///
    /// `P[i]` for `i in 0..=reset_step`, each in `(0, ∞)`; higher rate
    /// nodes produce smaller discount factors.
    ///
    /// # Errors
    ///
    /// Returns an error when the tree is uncalibrated, the steps are not
    /// strictly ordered within the lattice, or `time_to_maturity` differs
    /// from the calibrated horizon.
    pub fn conditional_discount_factors(
        &self,
        reset_step: usize,
        payment_step: usize,
        time_to_maturity: f64,
    ) -> Result<Vec<f64>> {
        let steps = self.config.steps;
        let dt = self.validate_pricing_horizon(time_to_maturity)?;
        if reset_step >= payment_step || payment_step > steps {
            return Err(Error::Validation(format!(
                "conditional discounting requires reset_step < payment_step <= steps, \
                 got reset_step={reset_step}, payment_step={payment_step}, steps={steps}"
            )));
        }
        // Backward induction on the rate marginal: value 1 at payment_step,
        // discount with the raw calibrated node rate, transition with the
        // same mean-reversion-aware probability pricing uses.
        let mut values = vec![1.0; payment_step + 1];
        for k in (reset_step..payment_step).rev() {
            let mut next = vec![0.0; k + 1];
            for (i, slot) in next.iter_mut().enumerate() {
                let r = self.calibrated_rates[k].value_unchecked(i);
                let p_up = Self::mean_reverting_up_prob(
                    r,
                    self.rate_ref,
                    self.config.rate_mean_reversion,
                    self.config.rate_vol,
                    dt,
                );
                let df = (-r * dt).exp();
                *slot = df * (p_up * values[i + 1] + (1.0 - p_up) * values[i]);
            }
            values = next;
        }
        Ok(values)
    }
}
