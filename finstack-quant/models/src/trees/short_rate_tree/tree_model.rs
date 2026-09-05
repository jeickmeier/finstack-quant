use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::{Error, HashMap, Result};

use crate::trees::hull_white_tree::HullWhiteTree;
use crate::trees::tree_framework::{
    price_recombining_tree, state_keys, CachedValues, NodeState, RecombiningInputs, TreeModel,
    TreeValuator,
};

use super::black_karasinski::BkTrinomialLattice;
use super::{short_rate_keys, ShortRateTree};

impl ShortRateTree {
    /// Backward induction over the Black-Karasinski trinomial lattice.
    ///
    /// Honors the per-node transition probabilities, the Hull & White edge
    /// branch switching, and the configured per-node compounding. OAS is an
    /// independently compounded continuous spread, so periodic node-rate
    /// conventions do not distort its discount factor.
    fn price_bk_trinomial<V: TreeValuator>(
        &self,
        lattice: &BkTrinomialLattice,
        initial_vars: &HashMap<&'static str, f64>,
        time_to_maturity: f64,
        market_context: &MarketContext,
        valuator: &V,
        oas: f64,
    ) -> Result<f64> {
        let steps = self.config.steps;
        let dt = time_to_maturity / steps as f64;
        let comp = self.config.compounding;
        let oas_shift = oas / 10_000.0;
        let j_max = lattice.j_max;

        let cached_hazard = initial_vars.get(state_keys::HAZARD_RATE).copied();
        let cached_spot = initial_vars.get(state_keys::SPOT).copied();
        let cached_df = initial_vars.get(state_keys::DF).copied();
        let cached_for = |rate: f64| -> CachedValues {
            CachedValues {
                spot: cached_spot,
                interest_rate: Some(rate),
                hazard_rate: cached_hazard,
                df: cached_df,
            }
        };

        let mut values: Vec<f64> = Vec::with_capacity(self.rates[steps].len());
        for &r in self.rates[steps].iter() {
            let state = NodeState::with_cached(
                steps,
                time_to_maturity,
                initial_vars,
                market_context,
                cached_for(r + oas_shift),
            );
            values.push(valuator.value_at_maturity(&state)?);
        }

        let mut scratch: Vec<f64> = Vec::new();
        for step in (0..steps).rev() {
            let curr_j_max = step.min(j_max);
            let next_j_max = (step + 1).min(j_max);
            let num_nodes = 2 * curr_j_max + 1;
            let boundary_j_max = if curr_j_max == next_j_max {
                curr_j_max
            } else {
                usize::MAX
            };
            let time_t = step as f64 * dt;

            scratch.clear();
            for j in 0..num_nodes {
                let j_signed = j as i32 - curr_j_max as i32;
                let node_probs = lattice.probs[step][j];

                let mut expected_value = 0.0;
                for (offset, probability) in
                    HullWhiteTree::transition_offsets(j_signed, boundary_j_max, node_probs)
                {
                    if let Some(idx) = HullWhiteTree::transition_index(j_signed, offset, next_j_max)
                    {
                        if idx < values.len() {
                            expected_value += probability * values[idx];
                        }
                    }
                }

                let r = self.rates[step][j];
                let continuation = expected_value * comp.df(r, dt) * (-oas_shift * dt).exp();
                let state = NodeState::with_cached(
                    step,
                    time_t,
                    initial_vars,
                    market_context,
                    cached_for(r + oas_shift),
                );
                scratch.push(valuator.value_at_node(&state, continuation, dt)?);
            }
            std::mem::swap(&mut values, &mut scratch);
        }

        values.first().copied().ok_or_else(|| {
            Error::internal("Black-Karasinski backward induction produced no root value")
        })
    }
}

impl TreeModel for ShortRateTree {
    fn price<V: TreeValuator>(
        &self,
        mut initial_vars: HashMap<&'static str, f64>,
        time_to_maturity: f64,
        market_context: &MarketContext,
        valuator: &V,
    ) -> Result<f64> {
        if self.rates.is_empty() {
            tracing::debug!("ShortRateTree::price called before calibration (rates is empty)");
            return Err(Error::internal(
                "short-rate tree must be calibrated before pricing",
            ));
        }
        self.validate_lattice_geometry()?;

        if !initial_vars.contains_key(state_keys::INTEREST_RATE) {
            if let Some(&r0) = self.rates.first().and_then(|row| row.first()) {
                initial_vars.insert(state_keys::INTEREST_RATE, r0);
            }
        }

        // OAS in basis points from initial variables (default to 0)
        let oas = initial_vars
            .get(short_rate_keys::OAS)
            .copied()
            .unwrap_or(0.0);

        // Black-Karasinski trinomial lattice: per-node probabilities and
        // capped width with branch switching cannot be expressed through the
        // constant-probability recombining engine, so it has a dedicated
        // backward induction.
        if let Some(lattice) = &self.bk_trinomial {
            return self.price_bk_trinomial(
                lattice,
                &initial_vars,
                time_to_maturity,
                market_context,
                valuator,
                oas,
            );
        }

        // Clone rates (cheap Arc clone) to avoid lifetime issues with closures
        let rates_clone = std::sync::Arc::clone(&self.rates);
        let state_gen: Box<dyn Fn(usize, usize) -> f64> =
            Box::new(move |step: usize, node: usize| -> f64 {
                if step < rates_clone.len() && node < rates_clone[step].len() {
                    rates_clone[step][node]
                } else {
                    0.0
                }
            });

        let rates_clone2 = std::sync::Arc::clone(&self.rates);
        let compounding = self.config.compounding;
        let dt_pricing = time_to_maturity / self.config.steps as f64;
        let rate_gen: Box<dyn Fn(usize, usize) -> f64> =
            Box::new(move |step: usize, node: usize| -> f64 {
                let r = if step < rates_clone2.len() && node < rates_clone2[step].len() {
                    rates_clone2[step][node]
                } else {
                    return 0.0;
                };
                compounding.to_continuous(r, dt_pricing) + oas / 10000.0
            });

        // Ho-Lee and binomial BDT lattices are calibrated with equal
        // up/down probabilities; the drift lives in the calibrated node rates.
        price_recombining_tree(RecombiningInputs {
            steps: self.config.steps,
            initial_vars,
            time_to_maturity,
            market_context,
            valuator,
            up_factor: 1.0,   // Not used with custom_state_generator
            down_factor: 1.0, // Not used with custom_state_generator
            prob_up: 0.5,
            prob_down: 0.5,
            interest_rate: 0.0, // Not used with custom_rate_generator
            custom_state_generator: Some(&*state_gen),
            custom_rate_generator: Some(&*rate_gen),
        })
    }
}
