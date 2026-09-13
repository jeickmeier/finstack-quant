//! Backward induction with node coupons and the `TreeModel` implementation.

use super::*;

impl RatesCreditTree {
    /// Pricing-measure fold of one node coupon's increment onto its reset
    /// slice.
    ///
    /// Returns the amount to add to the **continuation value** at each
    /// joint node `(i, j)` of `coupon.reset_step` (flattened with stride
    /// `max_nodes = steps + 1`). The unit claim — 1 paid at
    /// `payment_step` contingent on survival — is rolled back with exactly
    /// the operator the main backward induction applies to a deterministic
    /// cashflow at that slice: per-step discounting at the raw calibrated
    /// rate **plus the active OAS**, per-step survival at the floored node
    /// hazard, and the correlated joint transitions. The valuator applies the
    /// reset slice's own survival weighting when it wraps the continuation, so
    /// this fold deliberately stops one survival factor short at the reset
    /// slice. The payment-slice claim always seeds at `1`: cash paid at slice
    /// `m` must not be survival-weighted over the following interval
    /// `m -> m + 1`, whether or not `m` is terminal.
    ///
    /// The increment amount per rate node is `ΔC(i)` as documented on
    /// [`NodeCoupon`]; multiplying the unit fold by `ΔC(i)` is exact
    /// because the amount is fixed at the reset node.
    pub(super) fn folded_increment_values(
        &self,
        coupon: &NodeCoupon,
        time_to_maturity: f64,
        oas_decimal: f64,
    ) -> Result<Vec<f64>> {
        let steps = self.config.steps;
        let dt = self.validate_pricing_horizon(time_to_maturity)?;
        let max_nodes = steps + 1;
        let n = coupon.reset_step;
        let m = coupon.payment_step;

        // Node-dependent increment amounts from the forward-derivation
        // operator (raw rates, no OAS, no survival).
        let p_rf = self.conditional_discount_factors(n, m, time_to_maturity)?;
        let base_composed = calculate_floating_rate(coupon.base_index_rate, &coupon.params);
        let mut delta_amounts = vec![0.0; n + 1];
        for (i, slot) in delta_amounts.iter_mut().enumerate() {
            let node_forward = (1.0 / p_rf[i] - 1.0) / coupon.accrual;
            let delta_f = node_forward - coupon.base_discount_forward;
            let bumped = calculate_floating_rate(coupon.base_index_rate + delta_f, &coupon.params);
            *slot =
                coupon.notional * coupon.accrual * coupon.timing_scale * (bumped - base_composed);
        }

        // Unit-claim roll-back on the joint lattice with the pricing
        // operator. `w` holds the claim value at the slice currently being
        // consumed, in the same "post-valuator" form the main induction's
        // value function carries.
        let mut w = vec![0.0; max_nodes * max_nodes];
        let mut w_next = vec![0.0; max_nodes * max_nodes];
        for i in 0..=m {
            for j in 0..=m {
                w[i * max_nodes + j] = 1.0;
            }
        }
        for k in (n + 1..m).rev() {
            for i in 0..=k {
                let r = self.calibrated_rates[k].value_unchecked(i);
                let p_r = Self::mean_reverting_up_prob(
                    r,
                    self.rate_ref,
                    self.config.rate_mean_reversion,
                    self.config.rate_vol,
                    dt,
                );
                let df = (-(r + oas_decimal) * dt).exp();
                for j in 0..=k {
                    let h = self.calibrated_hazards[k].value_unchecked(j);
                    let p_h = Self::mean_reverting_up_prob(
                        h,
                        self.hazard_ref,
                        self.config.hazard_mean_reversion,
                        self.config.hazard_vol,
                        dt,
                    );
                    let (p_uu, p_ud, p_du, p_dd) = self.joint_probabilities(p_r, p_h);
                    let expectation = p_uu * w[(i + 1) * max_nodes + (j + 1)]
                        + p_ud * w[(i + 1) * max_nodes + j]
                        + p_du * w[i * max_nodes + (j + 1)]
                        + p_dd * w[i * max_nodes + j];
                    let p_surv = (-Self::effective_hazard(h) * dt).exp();
                    w_next[i * max_nodes + j] = p_surv * df * expectation;
                }
            }
            std::mem::swap(&mut w, &mut w_next);
        }

        // Assemble at the reset slice: continuation-form (discount + joint
        // expectation, no survival — the valuator applies it), scaled by the
        // node's increment amount.
        let mut folded = vec![0.0; max_nodes * max_nodes];
        for (i, &delta) in delta_amounts.iter().enumerate() {
            let r = self.calibrated_rates[n].value_unchecked(i);
            let p_r = Self::mean_reverting_up_prob(
                r,
                self.rate_ref,
                self.config.rate_mean_reversion,
                self.config.rate_vol,
                dt,
            );
            let df = (-(r + oas_decimal) * dt).exp();
            for j in 0..=n {
                let h = self.calibrated_hazards[n].value_unchecked(j);
                let p_h = Self::mean_reverting_up_prob(
                    h,
                    self.hazard_ref,
                    self.config.hazard_mean_reversion,
                    self.config.hazard_vol,
                    dt,
                );
                let (p_uu, p_ud, p_du, p_dd) = self.joint_probabilities(p_r, p_h);
                let expectation = p_uu * w[(i + 1) * max_nodes + (j + 1)]
                    + p_ud * w[(i + 1) * max_nodes + j]
                    + p_du * w[i * max_nodes + (j + 1)]
                    + p_dd * w[i * max_nodes + j];
                folded[i * max_nodes + j] = delta * df * expectation;
            }
        }
        Ok(folded)
    }
}

impl RatesCreditTree {
    /// Price with node-dependent floating-coupon increments folded into
    /// continuation at their reset slices.
    ///
    /// This is the full backward induction of [`TreeModel::price`] plus, at
    /// each [`NodeCoupon::reset_step`], the coupon's node-dependent
    /// increment added to the continuation value **before** the valuator's
    /// exercise decision and survival weighting. Adding it to continuation
    /// gives the correct exercise economics: a redemption at the reset
    /// slice forfeits the not-yet-accrued coupon, while a redemption at
    /// the payment slice still pays it (matching the engine's
    /// coupon-paid-regardless convention). Because the folded unit claim
    /// carries no exercise decisions between reset and payment, callers
    /// must ensure no exercise step lies strictly inside a node coupon's
    /// `(reset_step, payment_step)` — the instrument engines validate
    /// this before pricing.
    ///
    /// Two distinct operators are involved, per the callable-integration
    /// design:
    ///
    /// 1. **Forward derivation** — raw calibrated rates, no OAS, no
    ///    survival ([`Self::conditional_discount_factors`]); sets the
    ///    coupon amount at each rate node.
    /// 2. **Pricing-measure folding** — the same per-node discounting the
    ///    main induction applies (raw rate + OAS, floored-hazard survival,
    ///    correlated joint transitions); moves that amount from payment to
    ///    reset.
    ///
    /// With an empty `node_coupons` slice this is exactly
    /// [`TreeModel::price`], which delegates here.
    ///
    /// # Arguments
    ///
    /// * `initial_vars` - initial state variables; `"oas"` (basis points)
    ///   is applied as a parallel shift to the calibrated short rates
    /// * `time_to_maturity` - total lattice horizon in years
    /// * `market_context` - market data passed through to the valuator
    /// * `valuator` - instrument value function driven by the induction
    /// * `node_coupons` - future floating-coupon increments to fold at
    ///   their reset slices
    ///
    /// # Errors
    ///
    /// Returns an error when the tree is uncalibrated, the supplied horizon
    /// differs from calibration, a descriptor violates its invariants (see
    /// [`NodeCoupon`]), or the valuator fails.
    pub fn price_with_node_coupons<V: TreeValuator>(
        &self,
        initial_vars: HashMap<&'static str, f64>,
        time_to_maturity: f64,
        market_context: &MarketContext,
        valuator: &V,
        node_coupons: &[NodeCoupon],
    ) -> Result<f64> {
        let steps = self.config.steps;
        let dt = self.validate_pricing_horizon(time_to_maturity)?;

        // OAS from initial variables (bp units, same convention as ShortRateTree)
        let oas_decimal = initial_vars
            .get(short_rate_keys::OAS)
            .copied()
            .unwrap_or(0.0)
            / 10_000.0;

        // Fold every node coupon's increment onto its reset slice up front;
        // the claims are independent of the instrument value function, so
        // they precompute cleanly. The fold depends on the active OAS, so
        // an OAS solve re-folds on every objective evaluation by design.
        let mut folded_by_step: Vec<Option<Vec<f64>>> = vec![None; steps];
        for coupon in node_coupons {
            coupon.validate(steps)?;
            let folded = self.folded_increment_values(coupon, time_to_maturity, oas_decimal)?;
            match &mut folded_by_step[coupon.reset_step] {
                Some(existing) => {
                    for (slot, add) in existing.iter_mut().zip(folded.iter()) {
                        *slot += add;
                    }
                }
                slot @ None => *slot = Some(folded),
            }
        }

        // Pre-allocate flat double buffers for backward induction (zero
        // allocations in the loop). Row-major `[i * max_nodes + j]` storage is
        // cache-friendlier than a `Vec<Vec<f64>>` (no per-row pointer chase).
        let max_nodes = steps + 1;
        let mut curr_values: Vec<f64> = vec![0.0; max_nodes * max_nodes];
        let mut next_values: Vec<f64> = vec![0.0; max_nodes * max_nodes];

        // No valuator used with this tree reads node coordinates from
        // `state.vars`; they consume the cached `interest_rate`/`hazard_rate`
        // fields and `state.step`. Build each `NodeState` via `with_cached`
        // (supplying the per-node values directly) and skip the per-node
        // `HashMap` writes entirely — `initial_vars` passes through unchanged.
        for i in 0..=steps {
            let r_t = self.calibrated_rates[steps].value_unchecked(i);
            for j in 0..=steps {
                let h_t = self.calibrated_hazards[steps].value_unchecked(j);
                let cached = CachedValues {
                    interest_rate: Some(r_t.max(1e-8)),
                    hazard_rate: Some(Self::effective_hazard(h_t)),
                    ..CachedValues::default()
                };
                let state = NodeState::with_cached(
                    steps,
                    time_to_maturity,
                    &initial_vars,
                    market_context,
                    cached,
                );
                curr_values[i * max_nodes + j] = valuator.value_at_maturity(&state)?;
            }
        }

        for k in (0..steps).rev() {
            for i in 0..=k {
                let r_t = self.calibrated_rates[k].value_unchecked(i);

                // Rate transition probability with mean reversion. This is the
                // SAME function used during calibration; using an identical
                // per-node probability is what makes the tree reprice the
                // discount curve when mean reversion is non-zero.
                let p_r = Self::mean_reverting_up_prob(
                    r_t,
                    self.rate_ref,
                    self.config.rate_mean_reversion,
                    self.config.rate_vol,
                    dt,
                );

                for j in 0..=k {
                    let h_t = self.calibrated_hazards[k].value_unchecked(j);

                    // Hazard transition probability with mean reversion (same
                    // function and reference level used during calibration).
                    let p_h = Self::mean_reverting_up_prob(
                        h_t,
                        self.hazard_ref,
                        self.config.hazard_mean_reversion,
                        self.config.hazard_vol,
                        dt,
                    );

                    let (p_uu, p_ud, p_du, p_dd) = self.joint_probabilities(p_r, p_h);

                    let v_uu = curr_values[(i + 1) * max_nodes + (j + 1)];
                    let v_ud = curr_values[(i + 1) * max_nodes + j];
                    let v_du = curr_values[i * max_nodes + (j + 1)];
                    let v_dd = curr_values[i * max_nodes + j];

                    // Risk-free discounting with calibrated rate + OAS.
                    //
                    // The rate is NOT floored here: `calibrate_factor_ho_lee`
                    // discounts with the raw (un-floored) calibrated rate, so
                    // backward induction must do the same or the tree will not
                    // reprice the discount curve once a wide lattice produces
                    // negative node rates. The `1e-8` floor is still applied
                    // below to the INTEREST_RATE / HAZARD_RATE *state
                    // variables*, which shields valuators that cannot accept
                    // non-positive rates.
                    let df = (-(r_t + oas_decimal) * dt).exp();
                    let mut cont = df * (p_uu * v_uu + p_ud * v_ud + p_du * v_du + p_dd * v_dd);

                    // Node-dependent floating-coupon increments fold into
                    // continuation at their reset slice, ahead of the
                    // valuator's exercise decision and survival weighting.
                    if let Some(folded) = folded_by_step.get(k).and_then(|f| f.as_ref()) {
                        cont += folded[i * max_nodes + j];
                    }

                    // `r_t`/`h_t` are floored only for the cached *state*
                    // variables (shields valuators that reject non-positive
                    // rates); the discounting above intentionally uses the raw
                    // calibrated rate.
                    let cached = CachedValues {
                        interest_rate: Some(r_t.max(1e-8)),
                        hazard_rate: Some(Self::effective_hazard(h_t)),
                        df: Some(df),
                        ..CachedValues::default()
                    };
                    let state = NodeState::with_cached(
                        k,
                        k as f64 * dt,
                        &initial_vars,
                        market_context,
                        cached,
                    );
                    next_values[i * max_nodes + j] = valuator.value_at_node(&state, cont, dt)?;
                }
            }
            std::mem::swap(&mut curr_values, &mut next_values);
        }

        Ok(curr_values[0])
    }
}

impl TreeModel for RatesCreditTree {
    fn price<V: TreeValuator>(
        &self,
        initial_vars: HashMap<&'static str, f64>,
        time_to_maturity: f64,
        market_context: &MarketContext,
        valuator: &V,
    ) -> Result<f64> {
        self.price_with_node_coupons(
            initial_vars,
            time_to_maturity,
            market_context,
            valuator,
            &[],
        )
    }
}
