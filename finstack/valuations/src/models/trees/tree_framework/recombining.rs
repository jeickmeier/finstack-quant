use finstack_core::market_data::context::MarketContext;
use finstack_core::HashMap;
use finstack_core::Result;

use super::evolution::{BarrierSpec, BarrierStyle, StateGenerator, TreeBranching};
use super::node_state::{BarrierState, BarrierType, CachedValues, NodeState, StateVariables};
use super::state_keys;
use super::traits::TreeValuator;

/// Shared recombining tree engine that performs backward induction given constant
/// per-step evolution parameters and a branching policy.
#[derive(Clone)]
pub struct RecombiningInputs<'a, V: TreeValuator> {
    /// Branching structure (binomial or trinomial)
    pub branching: TreeBranching,
    /// Number of time steps in the tree
    pub steps: usize,
    /// Initial state variable values at root node
    pub initial_vars: StateVariables,
    /// Time to maturity in years
    pub time_to_maturity: f64,
    /// Market data context for curve lookups
    pub market_context: &'a MarketContext,
    /// Payoff valuator implementing TreeValuator trait
    pub valuator: &'a V,
    /// Multiplicative factor for up move (e.g., exp(σ√dt))
    pub up_factor: f64,
    /// Multiplicative factor for down move (e.g., exp(-σ√dt))
    pub down_factor: f64,
    /// Multiplicative factor for middle move (trinomial only)
    pub middle_factor: Option<f64>,
    /// Risk-neutral probability of up move
    pub prob_up: f64,
    /// Risk-neutral probability of down move
    pub prob_down: f64,
    /// Risk-neutral probability of middle move (trinomial only)
    pub prob_middle: Option<f64>,
    /// Risk-free interest rate per annum (used for discounting if custom_rate_generator is None)
    pub interest_rate: f64,
    /// Optional barrier configuration (discrete monitoring per step)
    pub barrier: Option<BarrierSpec>,
    /// Optional custom state generator for primary state variable (overrides up/down factors)
    pub custom_state_generator: Option<&'a StateGenerator>,
    /// Optional custom rate generator for discounting (overrides interest_rate)
    pub custom_rate_generator: Option<&'a StateGenerator>,
}

/// Price an option using a recombining tree with backward induction.
///
/// Supports binomial and trinomial trees with optional barrier monitoring.
/// The tree is built forward, payoffs are evaluated at maturity, and expected
/// values are discounted backward to the root.
///
/// # Arguments
///
/// * `inputs` - Complete tree configuration including evolution parameters,
///   valuator, and optional barrier specification
///
/// # Returns
///
/// Present value of the option at time 0
pub fn price_recombining_tree<V: TreeValuator>(inputs: RecombiningInputs<'_, V>) -> Result<f64> {
    let dt = inputs.time_to_maturity / inputs.steps as f64;

    // Pre-compute constant discount factor when no custom rate generator
    let const_df = if inputs.custom_rate_generator.is_none() {
        Some((-inputs.interest_rate * dt).exp())
    } else {
        None
    };

    // Helper: compute discount factor at a given step/node.
    // Either `const_df` or `custom_rate_generator` is set by the block above.
    let get_df = |step: usize, node: usize| -> f64 {
        if let Some(df) = const_df {
            df
        } else if let Some(rate_gen) = &inputs.custom_rate_generator {
            let r = rate_gen(step, node);
            (-r * dt).exp()
        } else {
            unreachable!("const_df is Some iff custom_rate_generator is None; both cannot be None")
        }
    };

    // Pre-compute ratio for incremental spot computation
    let ud_ratio = inputs.up_factor / inputs.down_factor;

    // Helper: compute state value (spot or rate) at a given step/node
    let get_state = |step: usize, node: usize, spot0: f64| -> f64 {
        if let Some(state_gen) = &inputs.custom_state_generator {
            state_gen(step, node)
        } else {
            // Default multiplicative evolution for binomial/trinomial
            match inputs.branching {
                TreeBranching::Binomial => {
                    // Node i at step n has i up moves and (n-i) down moves
                    let ups = node as i32;
                    let downs = step as i32 - node as i32;
                    spot0 * inputs.up_factor.powi(ups) * inputs.down_factor.powi(downs)
                }
                TreeBranching::Trinomial => {
                    // Trinomial tree: at step n, nodes j ∈ [0, 2n] with center at j=n
                    // j_centered = j - n ranges from -n to +n
                    // S(n,j) = S₀ * u^j_centered (since d = 1/u in standard setup)
                    //
                    // For generality (when d ≠ 1/u), we use:
                    // S(n,j) = S₀ * u^max(j_centered, 0) * d^max(-j_centered, 0)
                    let j_centered = node as i32 - step as i32;
                    if j_centered >= 0 {
                        spot0 * inputs.up_factor.powi(j_centered)
                    } else {
                        spot0 * inputs.down_factor.powi(-j_centered)
                    }
                }
            }
        }
    };

    // Hoist contains_key check: determine once which state key drives evolution.
    // The evolving primary state variable is either SPOT (equity trees) or
    // INTEREST_RATE (short-rate trees). The per-node value is threaded into the
    // matching cached field of `CachedValues` so no `HashMap` write is needed.
    let uses_spot_key = inputs.initial_vars.contains_key(state_keys::SPOT);
    let has_barrier = inputs.barrier.is_some();
    // Pre-extract scalars that stay constant across the whole tree. The evolving
    // node value (spot/rate) is the only thing that changes per node; everything
    // else is hoisted here so the backward-induction loop performs zero hashing.
    let cached_hazard = inputs.initial_vars.get(state_keys::HAZARD_RATE).copied();
    let const_spot = if uses_spot_key {
        None
    } else {
        inputs.initial_vars.get(state_keys::SPOT).copied()
    };
    let const_rate = if uses_spot_key {
        inputs.initial_vars.get(state_keys::INTEREST_RATE).copied()
    } else {
        None
    };
    let const_df = inputs.initial_vars.get(state_keys::DF).copied();

    // Build the `CachedValues` for a node given its evolving primary value.
    // `node_value` is the spot price (equity trees) or short rate (rate trees);
    // it is placed into the matching cached slot so `NodeState::with_cached`
    // sees identical values to what `NodeState::new` would extract from the map.
    let cached_for = |node_value: f64| -> CachedValues {
        if uses_spot_key {
            CachedValues {
                spot: Some(node_value),
                interest_rate: const_rate,
                hazard_rate: cached_hazard,
                df: const_df,
            }
        } else {
            CachedValues {
                spot: const_spot,
                interest_rate: Some(node_value),
                hazard_rate: cached_hazard,
                df: const_df,
            }
        }
    };

    // Helper: evaluate barrier touch at a given spot
    let barrier_touch = |spot: f64| -> (bool, bool, bool, f64) {
        if let Some(spec) = &inputs.barrier {
            let touched_up = spec.up_level.map(|lvl| spot >= lvl).unwrap_or(false);
            let touched_down = spec.down_level.map(|lvl| spot <= lvl).unwrap_or(false);
            let breached =
                matches!(spec.style, BarrierStyle::KnockOut) && (touched_up || touched_down);
            (touched_up, touched_down, breached, spec.rebate)
        } else {
            (false, false, false, 0.0)
        }
    };

    let barrier_is_knock_in = inputs
        .barrier
        .as_ref()
        .is_some_and(|spec| matches!(spec.style, BarrierStyle::KnockIn));

    match inputs.branching {
        TreeBranching::Binomial => {
            // Initialize terminal values
            let spot0 = *inputs
                .initial_vars
                .get(state_keys::SPOT)
                .or_else(|| inputs.initial_vars.get(state_keys::INTEREST_RATE))
                .ok_or_else(|| {
                    finstack_core::Error::internal(
                        "tree pricing requires initial SPOT or INTEREST_RATE state",
                    )
                })?;

            // `initial_vars` is never mutated during induction: its constant
            // keys (volatility, dividend_yield, credit_spread, ...) remain
            // available to valuators via `NodeState::get_var`, while the
            // evolving spot/rate value is threaded through `CachedValues`.
            let node_vars = &inputs.initial_vars;

            if barrier_is_knock_in {
                let spec = inputs.barrier.as_ref().ok_or_else(|| {
                    finstack_core::Error::internal(
                        "knock-in tree pricing requires a barrier specification",
                    )
                })?;
                let num_barriers =
                    spec.up_level.is_some() as usize + spec.down_level.is_some() as usize;
                if num_barriers != 1 {
                    return Err(finstack_core::Error::Validation(
                        "Knock-in tree pricing requires exactly one barrier (up or down)".into(),
                    ));
                }

                let (barrier_level, barrier_type) = if let Some(up) = spec.up_level {
                    (up, BarrierType::UpAndIn)
                } else if let Some(down) = spec.down_level {
                    (down, BarrierType::DownAndIn)
                } else {
                    return Err(finstack_core::Error::internal(
                        "knock-in tree pricing requires exactly one configured barrier level",
                    ));
                };
                let hit_state = BarrierState {
                    barrier_hit: true,
                    barrier_level,
                    barrier_type,
                };

                let mut hit_values = Vec::with_capacity(inputs.steps + 1);
                let mut not_hit_values = Vec::with_capacity(inputs.steps + 1);

                // Initialize terminal values with hit/not-hit states
                for i in 0..=inputs.steps {
                    let time_t = inputs.time_to_maturity;
                    let terminal_spot = get_state(inputs.steps, i, spot0);

                    let (t_up, t_dn, _breached, rebate) = barrier_touch(terminal_spot);
                    let touched = t_up || t_dn;

                    let terminal_state = NodeState::with_cached_barrier(
                        inputs.steps,
                        time_t,
                        node_vars,
                        inputs.market_context,
                        hit_state,
                        cached_for(terminal_spot),
                    );
                    let payoff_hit = inputs.valuator.value_at_maturity(&terminal_state)?;
                    let payoff_not_hit = if touched { payoff_hit } else { rebate };

                    hit_values.push(payoff_hit);
                    not_hit_values.push(payoff_not_hit);
                }

                // Backward induction with path-dependent barrier state
                // Reuse scratch vectors to avoid per-step allocation
                let mut next_hit = Vec::with_capacity(inputs.steps + 1);
                let mut next_not_hit = Vec::with_capacity(inputs.steps + 1);
                for step in (0..inputs.steps).rev() {
                    next_hit.clear();
                    next_not_hit.clear();
                    for i in 0..=step {
                        let spot_t = get_state(step, i, spot0);
                        let time_t = step as f64 * dt;
                        let df_node = get_df(step, i);

                        let (t_up, t_dn, _breached, _rebate) = barrier_touch(spot_t);
                        let touched = t_up || t_dn;

                        let continuation_hit = df_node
                            * (inputs.prob_up * hit_values[i + 1]
                                + inputs.prob_down * hit_values[i]);
                        let node_state_hit = NodeState::with_cached_barrier(
                            step,
                            time_t,
                            node_vars,
                            inputs.market_context,
                            hit_state,
                            cached_for(spot_t),
                        );
                        let value_hit =
                            inputs
                                .valuator
                                .value_at_node(&node_state_hit, continuation_hit, dt)?;

                        let value_not_hit = if touched {
                            value_hit
                        } else {
                            let spot_up = get_state(step + 1, i + 1, spot0);
                            let spot_down = get_state(step + 1, i, spot0);
                            let (up_t_up, up_t_dn, _up_breached, _up_rebate) =
                                barrier_touch(spot_up);
                            let (dn_t_up, dn_t_dn, _dn_breached, _dn_rebate) =
                                barrier_touch(spot_down);
                            let child_up_touched = up_t_up || up_t_dn;
                            let child_down_touched = dn_t_up || dn_t_dn;

                            let next_up = if child_up_touched {
                                hit_values[i + 1]
                            } else {
                                not_hit_values[i + 1]
                            };
                            let next_down = if child_down_touched {
                                hit_values[i]
                            } else {
                                not_hit_values[i]
                            };
                            df_node * (inputs.prob_up * next_up + inputs.prob_down * next_down)
                        };

                        next_hit.push(value_hit);
                        next_not_hit.push(value_not_hit);
                    }
                    std::mem::swap(&mut hit_values, &mut next_hit);
                    std::mem::swap(&mut not_hit_values, &mut next_not_hit);
                }

                return Ok(not_hit_values[0]);
            }

            let mut values = Vec::with_capacity(inputs.steps + 1);

            let use_incremental = inputs.custom_state_generator.is_none();

            // Initialize terminal values using custom state generator if provided
            if use_incremental {
                // Incremental spot computation: start from spot0 * d^N, multiply by u/d
                let mut terminal_spot = spot0 * inputs.down_factor.powi(inputs.steps as i32);
                for i in 0..=inputs.steps {
                    let time_t = inputs.time_to_maturity;
                    let terminal_state = NodeState::with_cached(
                        inputs.steps,
                        time_t,
                        node_vars,
                        inputs.market_context,
                        cached_for(terminal_spot),
                    );
                    if has_barrier {
                        let (_t_up, _t_dn, breached, rebate) = barrier_touch(terminal_spot);
                        values.push(if breached {
                            rebate
                        } else {
                            inputs.valuator.value_at_maturity(&terminal_state)?
                        });
                    } else {
                        values.push(inputs.valuator.value_at_maturity(&terminal_state)?);
                    }
                    if i < inputs.steps {
                        terminal_spot *= ud_ratio;
                    }
                }
            } else {
                for i in 0..=inputs.steps {
                    let time_t = inputs.time_to_maturity;
                    let terminal_spot = get_state(inputs.steps, i, spot0);
                    let terminal_state = NodeState::with_cached(
                        inputs.steps,
                        time_t,
                        node_vars,
                        inputs.market_context,
                        cached_for(terminal_spot),
                    );
                    if has_barrier {
                        let (_t_up, _t_dn, breached, rebate) = barrier_touch(terminal_spot);
                        values.push(if breached {
                            rebate
                        } else {
                            inputs.valuator.value_at_maturity(&terminal_state)?
                        });
                    } else {
                        values.push(inputs.valuator.value_at_maturity(&terminal_state)?);
                    }
                }
            }

            // Backward induction
            for step in (0..inputs.steps).rev() {
                let time_t = step as f64 * dt;

                if use_incremental {
                    // Incremental spot computation for this step
                    let mut spot_t = spot0 * inputs.down_factor.powi(step as i32);
                    for i in 0..=step {
                        let df_node = get_df(step, i);
                        let continuation = df_node
                            * (inputs.prob_up * values[i + 1] + inputs.prob_down * values[i]);

                        let node_state = NodeState::with_cached(
                            step,
                            time_t,
                            node_vars,
                            inputs.market_context,
                            cached_for(spot_t),
                        );

                        if has_barrier {
                            let (_t_up, _t_dn, breached, rebate) = barrier_touch(spot_t);
                            values[i] = if breached {
                                rebate
                            } else {
                                inputs
                                    .valuator
                                    .value_at_node(&node_state, continuation, dt)?
                            };
                        } else {
                            values[i] =
                                inputs
                                    .valuator
                                    .value_at_node(&node_state, continuation, dt)?;
                        }
                        if i < step {
                            spot_t *= ud_ratio;
                        }
                    }
                } else {
                    for i in 0..=step {
                        let spot_t = get_state(step, i, spot0);
                        let (_t_up, _t_dn, breached, rebate) = barrier_touch(spot_t);
                        let df_node = get_df(step, i);
                        let continuation = df_node
                            * (inputs.prob_up * values[i + 1] + inputs.prob_down * values[i]);

                        let node_state = NodeState::with_cached(
                            step,
                            time_t,
                            node_vars,
                            inputs.market_context,
                            cached_for(spot_t),
                        );
                        values[i] = if breached {
                            rebate
                        } else {
                            inputs
                                .valuator
                                .value_at_node(&node_state, continuation, dt)?
                        };
                    }
                }
                values.pop();
            }

            Ok(values[0])
        }
        TreeBranching::Trinomial => {
            let spot0 = *inputs
                .initial_vars
                .get(state_keys::SPOT)
                .or_else(|| inputs.initial_vars.get(state_keys::INTEREST_RATE))
                .ok_or_else(|| {
                    finstack_core::Error::internal(
                        "tree pricing requires initial SPOT or INTEREST_RATE state",
                    )
                })?;

            let p_m = inputs.prob_middle.unwrap_or(0.0);

            let max_nodes = 2 * inputs.steps + 1;
            // `initial_vars` stays immutable; the evolving spot/rate is threaded
            // through `CachedValues` so the induction loop performs zero hashing.
            let node_vars = &inputs.initial_vars;

            if barrier_is_knock_in {
                let spec = inputs.barrier.as_ref().ok_or_else(|| {
                    finstack_core::Error::internal(
                        "knock-in tree pricing requires a barrier specification",
                    )
                })?;
                let num_barriers =
                    spec.up_level.is_some() as usize + spec.down_level.is_some() as usize;
                if num_barriers != 1 {
                    return Err(finstack_core::Error::Validation(
                        "Knock-in tree pricing requires exactly one barrier (up or down)".into(),
                    ));
                }

                let (barrier_level, barrier_type) = if let Some(up) = spec.up_level {
                    (up, BarrierType::UpAndIn)
                } else if let Some(down) = spec.down_level {
                    (down, BarrierType::DownAndIn)
                } else {
                    return Err(finstack_core::Error::internal(
                        "knock-in tree pricing requires exactly one configured barrier level",
                    ));
                };
                let hit_state = BarrierState {
                    barrier_hit: true,
                    barrier_level,
                    barrier_type,
                };

                let mut hit_curr = vec![0.0; max_nodes];
                let mut hit_next = vec![0.0; max_nodes];
                let mut nothit_curr = vec![0.0; max_nodes];
                let mut nothit_next = vec![0.0; max_nodes];

                // Terminal values
                for j in 0..max_nodes {
                    if j <= 2 * inputs.steps {
                        let spot_t = get_state(inputs.steps, j, spot0);
                        let time_t = inputs.time_to_maturity;

                        let (t_up, t_dn, _breached, rebate) = barrier_touch(spot_t);
                        let touched = t_up || t_dn;

                        let terminal_state = NodeState::with_cached_barrier(
                            inputs.steps,
                            time_t,
                            node_vars,
                            inputs.market_context,
                            hit_state,
                            cached_for(spot_t),
                        );
                        let payoff_hit = inputs.valuator.value_at_maturity(&terminal_state)?;
                        let payoff_not_hit = if touched { payoff_hit } else { rebate };

                        hit_next[j] = payoff_hit;
                        nothit_next[j] = payoff_not_hit;
                    }
                }

                // Backward induction with double-buffer
                for step in (0..inputs.steps).rev() {
                    let nodes_at_step = 2 * step + 1;
                    for j in 0..nodes_at_step {
                        let spot_t = get_state(step, j, spot0);
                        let time_t = step as f64 * dt;
                        let df_node = get_df(step, j);

                        let up_idx = j + 2;
                        let mid_idx = j + 1;
                        let down_idx = j;

                        let (t_up, t_dn, _breached, _rebate) = barrier_touch(spot_t);
                        let touched = t_up || t_dn;

                        let continuation_hit = df_node
                            * (inputs.prob_up * hit_next[up_idx]
                                + p_m * hit_next[mid_idx]
                                + inputs.prob_down * hit_next[down_idx]);
                        let node_state_hit = NodeState::with_cached_barrier(
                            step,
                            time_t,
                            node_vars,
                            inputs.market_context,
                            hit_state,
                            cached_for(spot_t),
                        );
                        let value_hit =
                            inputs
                                .valuator
                                .value_at_node(&node_state_hit, continuation_hit, dt)?;

                        let value_not_hit = if touched {
                            value_hit
                        } else {
                            let spot_up = get_state(step + 1, up_idx, spot0);
                            let spot_mid = get_state(step + 1, mid_idx, spot0);
                            let spot_down = get_state(step + 1, down_idx, spot0);

                            let (up_t_up, up_t_dn, _up_breached, _up_rebate) =
                                barrier_touch(spot_up);
                            let (mid_t_up, mid_t_dn, _mid_breached, _mid_rebate) =
                                barrier_touch(spot_mid);
                            let (dn_t_up, dn_t_dn, _dn_breached, _dn_rebate) =
                                barrier_touch(spot_down);
                            let child_up_touched = up_t_up || up_t_dn;
                            let child_mid_touched = mid_t_up || mid_t_dn;
                            let child_down_touched = dn_t_up || dn_t_dn;

                            let next_up = if child_up_touched {
                                hit_next[up_idx]
                            } else {
                                nothit_next[up_idx]
                            };
                            let next_mid = if child_mid_touched {
                                hit_next[mid_idx]
                            } else {
                                nothit_next[mid_idx]
                            };
                            let next_down = if child_down_touched {
                                hit_next[down_idx]
                            } else {
                                nothit_next[down_idx]
                            };

                            df_node
                                * (inputs.prob_up * next_up
                                    + p_m * next_mid
                                    + inputs.prob_down * next_down)
                        };

                        hit_curr[j] = value_hit;
                        nothit_curr[j] = value_not_hit;
                    }
                    std::mem::swap(&mut hit_curr, &mut hit_next);
                    std::mem::swap(&mut nothit_curr, &mut nothit_next);
                }

                return Ok(nothit_next[0]);
            }

            let mut curr_buf = vec![0.0; max_nodes];
            let mut next_buf = vec![0.0; max_nodes];

            // Terminal values into next_buf
            #[allow(clippy::needless_range_loop)]
            for j in 0..max_nodes {
                if j <= 2 * inputs.steps {
                    let spot_t = get_state(inputs.steps, j, spot0);
                    let time_t = inputs.time_to_maturity;

                    let (_t_up, _t_dn, breached, rebate) = barrier_touch(spot_t);

                    let terminal_state = NodeState::with_cached(
                        inputs.steps,
                        time_t,
                        node_vars,
                        inputs.market_context,
                        cached_for(spot_t),
                    );
                    let payoff = if breached {
                        rebate
                    } else {
                        inputs.valuator.value_at_maturity(&terminal_state)?
                    };
                    next_buf[j] = payoff;
                }
            }

            // Backward induction with double-buffer
            #[allow(clippy::needless_range_loop)]
            for step in (0..inputs.steps).rev() {
                let nodes_at_step = 2 * step + 1;
                for j in 0..nodes_at_step {
                    let spot_t = get_state(step, j, spot0);
                    let time_t = step as f64 * dt;

                    let up_idx = j + 2;
                    let mid_idx = j + 1;
                    let down_idx = j;

                    let df_node = get_df(step, j);
                    let continuation = df_node
                        * (inputs.prob_up * next_buf[up_idx]
                            + p_m * next_buf[mid_idx]
                            + inputs.prob_down * next_buf[down_idx]);

                    let (_t_up, _t_dn, breached, rebate) = barrier_touch(spot_t);

                    let node_state = NodeState::with_cached(
                        step,
                        time_t,
                        node_vars,
                        inputs.market_context,
                        cached_for(spot_t),
                    );
                    curr_buf[j] = if breached {
                        rebate
                    } else {
                        inputs
                            .valuator
                            .value_at_node(&node_state, continuation, dt)?
                    };
                }
                std::mem::swap(&mut curr_buf, &mut next_buf);
            }

            Ok(next_buf[0])
        }
    }
}

/// Helper function to create initial state variables for single-factor equity model
pub fn single_factor_equity_state(
    spot: f64,
    risk_free_rate: f64,
    dividend_yield: f64,
    volatility: f64,
) -> StateVariables {
    let mut vars = HashMap::default();
    vars.insert(state_keys::SPOT, spot);
    vars.insert(state_keys::INTEREST_RATE, risk_free_rate);
    vars.insert(state_keys::DIVIDEND_YIELD, dividend_yield);
    vars.insert(state_keys::VOLATILITY, volatility);
    vars
}

/// Helper function to create initial state variables for two-factor model
pub fn two_factor_equity_rates_state(
    spot: f64,
    risk_free_rate: f64,
    dividend_yield: f64,
    equity_volatility: f64,
    rate_volatility: f64,
) -> StateVariables {
    let mut vars = HashMap::default();
    vars.insert(state_keys::SPOT, spot);
    vars.insert(state_keys::INTEREST_RATE, risk_free_rate);
    vars.insert(state_keys::DIVIDEND_YIELD, dividend_yield);
    vars.insert(state_keys::VOLATILITY, equity_volatility);
    vars.insert(state_keys::RATE_VOLATILITY, rate_volatility);
    vars
}
