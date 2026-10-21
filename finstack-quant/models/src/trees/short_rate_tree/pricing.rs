use finstack_quant_core::math::Compounding;
use finstack_quant_core::{Error, Result};

use crate::trees::tree_framework::{
    price_recombining_tree, NodeState, RecombiningInputs, RecombiningLattice, TreeDiscounting,
    TreeValuator,
};

use super::black_karasinski::{transition_index, transition_offsets, BkTrinomialLattice};
use super::{ShortRateModel, ShortRateTree};

impl ShortRateTree {
    /// Backward induction over the Black-Karasinski trinomial lattice.
    ///
    /// Honors the per-node transition probabilities, the Hull & White edge
    /// branch switching, and the configured per-node compounding. The OAS is
    /// added to each node rate on `oas_compounding` and applied as that
    /// node's continuous shift.
    fn price_bk_trinomial<V: TreeValuator>(
        &self,
        lattice: &BkTrinomialLattice,
        oas_bp: f64,
        oas_compounding: Compounding,
        valuator: &V,
    ) -> Result<f64> {
        let steps = self.config.steps;
        let dt = self.time_steps[1] - self.time_steps[0];
        let comp = self.config.compounding;
        let oas = oas_bp / 10_000.0;
        let j_max = lattice.j_max;
        let oas_shift_at = |rate: f64| -> f64 {
            oas_compounding.continuous_spread_shift(comp.tree_to_continuous(rate, dt), oas)
        };

        let state_for = |step: usize, rate: f64, oas_shift: f64| -> NodeState {
            NodeState {
                step,
                oas_bp: if oas_compounding == Compounding::Continuous {
                    oas_bp
                } else {
                    oas_shift * 10_000.0
                },
                interest_rate: Some(rate),
                ..NodeState::default()
            }
        };

        let mut values: Vec<f64> = Vec::with_capacity(self.rates[steps].len());
        for &r in self.rates[steps].iter() {
            let state = state_for(steps, r, oas_shift_at(r));
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

            scratch.clear();
            for j in 0..num_nodes {
                let j_signed = j as i32 - curr_j_max as i32;
                let node_probs = lattice.probs[step][j];

                let mut expected_value = 0.0;
                for (offset, probability) in
                    transition_offsets(j_signed, boundary_j_max, node_probs)
                {
                    if let Some(idx) = transition_index(j_signed, offset, next_j_max) {
                        if idx < values.len() {
                            expected_value += probability * values[idx];
                        }
                    }
                }

                let r = self.rates[step][j];
                let oas_shift = oas_shift_at(r);
                let continuation = expected_value * comp.tree_df(r, dt) * (-oas_shift * dt).exp();
                let state = state_for(step, r, oas_shift);
                scratch.push(valuator.value_at_node(&state, continuation, dt)?);
            }
            std::mem::swap(&mut values, &mut scratch);
        }

        values.first().copied().ok_or_else(|| {
            Error::Internal("Black-Karasinski backward induction produced no root value".into())
        })
    }
}

impl ShortRateTree {
    /// Price an instrument by backward induction over the calibrated lattice
    /// with a continuously compounded option-adjusted spread.
    ///
    /// Works for every [`ShortRateModel`]: Ho-Lee and Black-Derman-Toy roll
    /// back on the binomial lattice, Black-Karasinski on its trinomial one.
    /// The horizon is the one the tree was calibrated to.
    ///
    /// # Arguments
    ///
    /// * `oas_bp` - Option-adjusted spread in basis points, continuously
    ///   compounded, added to every node short rate for discounting and
    ///   passed to the valuator as [`NodeState::oas_bp`]. Zero prices on the
    ///   calibrated curve.
    /// * `valuator` - Instrument payoff and exercise logic applied at each
    ///   node.
    ///
    /// # Errors
    ///
    /// Returns an error when the tree has not been calibrated, its stored
    /// lattice is inconsistent with its configuration, or the valuator fails.
    #[must_use = "pricing result should not be discarded"]
    pub fn price<V: TreeValuator>(&self, oas_bp: f64, valuator: &V) -> Result<f64> {
        self.price_with_oas_compounding(oas_bp, Compounding::Continuous, valuator)
    }

    /// Price an instrument by backward induction with an option-adjusted
    /// spread quoted on `oas_compounding`.
    ///
    /// The spread is added to each node's short rate restated on
    /// `oas_compounding` (see [`Compounding::continuous_spread_shift`]), so
    /// a semiannual bond-equivalent OAS discounts each node at
    /// `1 + (z + oas) / 2` per half year. The valuator receives the node's
    /// continuous shift as [`NodeState::oas_bp`].
    ///
    /// # Arguments
    ///
    /// * `oas_bp` - Option-adjusted spread in basis points on
    ///   `oas_compounding`. Zero prices on the calibrated curve.
    /// * `oas_compounding` - Compounding basis of `oas_bp`;
    ///   [`Compounding::Continuous`] reproduces [`Self::price`].
    /// * `valuator` - Instrument payoff and exercise logic applied at each
    ///   node.
    ///
    /// # Errors
    ///
    /// Returns an error when the tree has not been calibrated, its stored
    /// lattice is inconsistent with its configuration, or the valuator fails.
    #[must_use = "pricing result should not be discarded"]
    pub fn price_with_oas_compounding<V: TreeValuator>(
        &self,
        oas_bp: f64,
        oas_compounding: Compounding,
        valuator: &V,
    ) -> Result<f64> {
        if self.rates.is_empty() {
            tracing::debug!("ShortRateTree::price called before calibration (rates is empty)");
            return Err(Error::Internal(
                "short-rate tree must be calibrated before pricing".into(),
            ));
        }
        self.validate_lattice_geometry()?;
        let time_to_maturity = self.time_steps.last().copied().ok_or_else(|| {
            Error::Internal("short-rate tree must be calibrated before pricing".into())
        })?;

        // Black-Karasinski trinomial lattice: per-node probabilities and
        // capped width with branch switching cannot be expressed through the
        // constant-probability recombining engine, so it has a dedicated
        // backward induction.
        if self.config.model == ShortRateModel::BlackKarasinski {
            let lattice = self.bk_trinomial.as_ref().ok_or_else(|| {
                Error::Internal("Black-Karasinski tree has no calibrated trinomial lattice".into())
            })?;
            return self.price_bk_trinomial(lattice, oas_bp, oas_compounding, valuator);
        }

        let rates = self.rates.as_slice();
        let compounding = self.config.compounding;
        let dt_pricing = self.time_steps[1] - self.time_steps[0];
        let oas = oas_bp / 10_000.0;
        let state_gen = |step: usize, node: usize| -> f64 {
            rates
                .get(step)
                .and_then(|row| row.get(node))
                .copied()
                .unwrap_or(0.0)
        };
        let rate_gen = |step: usize, node: usize| -> f64 {
            let Some(&r) = rates.get(step).and_then(|row| row.get(node)) else {
                return 0.0;
            };
            let continuous = compounding.tree_to_continuous(r, dt_pricing);
            continuous + oas_compounding.continuous_spread_shift(continuous, oas)
        };
        let node_oas_bp = |step: usize, node: usize| -> f64 {
            let Some(&r) = rates.get(step).and_then(|row| row.get(node)) else {
                return oas_bp;
            };
            if oas_compounding == Compounding::Continuous {
                return oas_bp;
            }
            oas_compounding
                .continuous_spread_shift(compounding.tree_to_continuous(r, dt_pricing), oas)
                * 10_000.0
        };

        // Ho-Lee and binomial BDT lattices are calibrated with equal
        // up/down probabilities; the drift lives in the calibrated node rates.
        price_recombining_tree(RecombiningInputs {
            steps: self.config.steps,
            time_to_maturity,
            valuator,
            prob_up: 0.5,
            prob_down: 0.5,
            lattice: RecombiningLattice::ShortRate {
                node_rate: &state_gen,
                discount_rate: &rate_gen,
                oas_bp: &node_oas_bp,
            },
        })
    }
}
