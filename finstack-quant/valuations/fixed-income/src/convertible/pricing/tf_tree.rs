//! Tsiveriotis-Fernandes convertible lattice.
//!
//! The convertible is split into an equity component, which the issuer can
//! always deliver by issuing shares, and a cash-only component (coupons,
//! redemption, call and put cash), which carries issuer credit risk. Each
//! lattice node carries the value `V` and a conversion probability `p`. A
//! branch is discounted at the risk-free step factor times
//! `exp(-(1 - p_child) * s * dt)`, so a claim that will be converted with
//! certainty is discounted risk-free and a pure cash claim at `r + s`. There is
//! no default event on the lattice and the stock has drift `r - q`.
//!
//! This is the blended-rate form QuantLib's `BinomialConvertibleEngine` uses
//! (`TsiveriotisFernandesLattice`), on this crate's conventions: per-step
//! discount factors from the curve with continuous compounding, where QuantLib
//! uses one flat rate with simple per-step compounding. The two differ at
//! `O(dt)` and converge with the step count.
//!
//! The per-step credit spread is derived from the bond's hazard curve and
//! recovery rate, `s_i * dt = -ln(Q_i) * (1 - R)`, with `Q_i` the conditional
//! survival over the step.
//!
//! Exercise at a node uses the same contract as the defaultable-equity tree in
//! `credit_tree`: conversion, then issuer call, then holder put, with the
//! coupon paid whatever the decision. A cash call or put resets `p` to zero,
//! because the resulting claim is cash only; QuantLib leaves `p` unchanged
//! there.
//!
//! On conversion `p` takes the share of the node's lattice cell that converts
//! rather than jumping to one, so the price stays continuous in the share
//! price as the conversion boundary crosses the lattice. The value itself
//! takes the node's own decision, exactly as the defaultable-equity tree does,
//! so with no credit spread the two lattices give the same price bit for bit.
//!
//! # References
//!
//! - Tsiveriotis, K., & Fernandes, C. (1998). "Valuing Convertible Bonds with
//!   Credit Risk." `docs/REFERENCES.md#tsiveriotis-fernandes-1998`

use finstack_quant_core::{Error, Result};

use finstack_quant_models::trees::tree_framework::EvolutionParams;

use super::credit_tree::{CreditTreeMarketInputs, terminal_payoff};
use super::valuator::ConvertibleBondValuator;

/// Conversion probability implied by a `terminal_payoff` split: the share of
/// the exercise value that is not cash.
fn equity_share(cash_ex_coupon: f64, cash_level: f64) -> f64 {
    if cash_level > 0.0 {
        (1.0 - cash_ex_coupon / cash_level).clamp(0.0, 1.0)
    } else {
        1.0
    }
}

/// A node after the backward step and before its exercise decisions.
#[derive(Clone, Copy)]
struct RolledNode {
    /// Discounted expected value of holding.
    value: f64,
    /// Expected conversion probability of holding.
    probability: f64,
    /// Share price at the node.
    spot: f64,
    /// Value of converting at the node.
    conversion: f64,
    /// Whether conversion is permitted at the node.
    can_convert: bool,
}

/// Share of node `index`'s lattice cell on which conversion beats holding.
///
/// The cell reaches half a node spacing either side. With the conversion
/// advantage `conversion - hold` taken as linear between adjacent nodes, a
/// sign change against a neighbour puts the conversion boundary a fraction
/// `|own| / (|own| + |neighbour|)` of the way towards it; the part of the half
/// cell beyond the boundary belongs to the other decision. A node whose
/// advantage keeps its sign against both neighbours, or whose neighbour cannot
/// convert, has the plain zero-or-one share.
fn converting_share(level: &[RolledNode], index: usize) -> f64 {
    let advantage = |node: &RolledNode| node.conversion - node.value;
    let own = advantage(&level[index]);
    let converts = own > 0.0;
    let mut share = if converts { 1.0 } else { 0.0 };
    let neighbours = [index.checked_sub(1), index.checked_add(1)];
    for neighbour in neighbours.into_iter().flatten() {
        let Some(node) = level.get(neighbour) else {
            continue;
        };
        let other = advantage(node);
        if !node.can_convert || (other > 0.0) == converts || !other.is_finite() {
            continue;
        }
        let boundary = own.abs() / (own.abs() + other.abs());
        let beyond = (0.5 - boundary).max(0.0);
        share += if converts { -beyond } else { beyond };
    }
    share.clamp(0.0, 1.0)
}

pub(super) struct TsiveriotisFernandesEngine<'a> {
    pub(super) valuator: &'a ConvertibleBondValuator,
    pub(super) steps: usize,
    pub(super) time_to_maturity: f64,
    /// Recovery rate as a decimal fraction of principal, in `[0, 1]`.
    pub(super) recovery_rate: f64,
}

impl TsiveriotisFernandesEngine<'_> {
    /// Per-step credit discount factor `exp(-s_i * dt)` of a pure cash claim.
    fn credit_step_dfs(&self) -> Result<Vec<f64>> {
        let loss = 1.0 - self.recovery_rate;
        self.valuator
            .survival_factors
            .iter()
            .map(|&survival| {
                if survival > 0.0 && survival.is_finite() {
                    Ok(survival.powf(loss))
                } else {
                    Err(Error::Validation(format!(
                        "Tsiveriotis-Fernandes tree requires positive per-step survival, got {survival}"
                    )))
                }
            })
            .collect()
    }

    /// Present value of the convertible.
    pub(super) fn price(&self, market: CreditTreeMarketInputs) -> Result<f64> {
        let CreditTreeMarketInputs {
            spot,
            volatility,
            risk_free_rate,
            dividend_yield,
        } = market;
        let dt = self.time_to_maturity / self.steps as f64;
        let credit_dfs = self.credit_step_dfs()?;

        let protected_dividend_rate = if self.valuator.dividend_protected {
            dividend_yield.max(0.0)
        } else {
            0.0
        };
        let ratio_accretion_at = |step: usize| (protected_dividend_rate * step as f64 * dt).exp();

        // Constant CRR up/down factors keep the lattice recombining. The stock
        // drifts at the risk-free forward of each step less the dividend
        // yield: credit enters through discounting only.
        let params = EvolutionParams::equity_crr(volatility, risk_free_rate, dividend_yield, dt)?;
        let mut step_params = Vec::with_capacity(self.steps);
        for &df in &self.valuator.rf_step_dfs {
            if df <= 0.0 {
                return Err(Error::Validation(format!(
                    "convertible tree requires positive per-step risk-free discount \
                     factors, got {df}"
                )));
            }
            step_params.push(params.with_drift(-df.ln() / dt - dividend_yield, dt)?);
        }

        let get_spot = |step: usize, node: usize| -> f64 {
            let ups = node as i32;
            let downs = step as i32 - node as i32;
            spot * params.up_factor.powi(ups) * params.down_factor.powi(downs)
        };

        // State per node: (value, conversion probability).
        let num_nodes = self.steps + 1;
        let mandatory = self.valuator.conversion_is_mandatory();
        let terminal_accretion = ratio_accretion_at(self.steps);
        let terminal_coupon = self
            .valuator
            .coupon_map
            .get(&self.steps)
            .copied()
            .unwrap_or(0.0);
        let terminal_put = self.valuator.put_price_at_step(self.steps);
        let terminal_log_spacing = if num_nodes > 1 {
            (get_spot(self.steps, 1) / get_spot(self.steps, 0)).ln()
        } else {
            0.0
        };
        let principal = self
            .valuator
            .principal_map
            .get(&self.steps)
            .copied()
            .unwrap_or(0.0);
        let cash_level = principal.max(terminal_put.unwrap_or(principal));

        let mut values: Vec<(f64, f64)> = Vec::with_capacity(num_nodes);
        for i in 0..num_nodes {
            let node_spot = get_spot(self.steps, i);
            let conversion_val =
                self.valuator
                    .conversion_value(node_spot, terminal_accretion, self.steps);
            let (total, cash) = terminal_payoff(
                conversion_val,
                cash_level,
                terminal_coupon,
                self.valuator.conversion_allowed(self.steps, node_spot),
                mandatory,
                terminal_log_spacing,
            );
            values.push((total, equity_share(cash - terminal_coupon, cash_level)));
        }

        let mut next_values: Vec<(f64, f64)> = Vec::with_capacity(values.len());
        let mut rolled: Vec<RolledNode> = Vec::with_capacity(values.len());
        for step in (0..self.steps).rev() {
            let rf_df = self.valuator.rf_step_dfs[step];
            let credit_df = credit_dfs[step];
            let sp = &step_params[step];
            let step_accretion = ratio_accretion_at(step);
            let coupon = self.valuator.coupon_map.get(&step).copied().unwrap_or(0.0)
                + self
                    .valuator
                    .principal_map
                    .get(&step)
                    .copied()
                    .unwrap_or(0.0);
            let call_price_here = self.valuator.call_price_at_step(step);
            let put_price_here = self.valuator.put_price_at_step(step);

            // First pass: roll every node back and record what conversion
            // would be worth there.
            rolled.clear();
            for i in 0..=step {
                let (v_up, p_up) = values[i + 1];
                let (v_down, p_down) = values[i];
                // Each branch carries the credit discount of its own child:
                // nothing when conversion there is certain, the full spread
                // when the child is a pure cash claim.
                let branch =
                    |value: f64, probability: f64| value * credit_df.powf(1.0 - probability);
                let expected =
                    sp.prob_up * branch(v_up, p_up) + sp.prob_down * branch(v_down, p_down);
                let node_spot = get_spot(step, i);
                rolled.push(RolledNode {
                    value: expected * rf_df,
                    probability: sp.prob_up * p_up + sp.prob_down * p_down,
                    spot: node_spot,
                    conversion: self
                        .valuator
                        .conversion_value(node_spot, step_accretion, step),
                    can_convert: self.valuator.conversion_allowed(step, node_spot),
                });
            }

            // Second pass: exercise decisions.
            next_values.clear();
            for i in 0..=step {
                let RolledNode {
                    mut value,
                    mut probability,
                    spot: node_spot,
                    conversion: conversion_val,
                    can_convert,
                } = rolled[i];

                if can_convert && mandatory {
                    // A forced delivery extinguishes the bond; call and put
                    // alternatives must not overwrite it.
                    let (total, _) = terminal_payoff(
                        conversion_val,
                        self.valuator.face_value,
                        coupon,
                        true,
                        true,
                        0.0,
                    );
                    next_values.push((total, 1.0));
                    continue;
                }
                if can_convert {
                    // The value takes the node's own decision. The
                    // conversion probability takes the share of the node's
                    // cell that converts, so it moves continuously as the
                    // conversion boundary crosses the lattice instead of
                    // jumping between zero and one.
                    let share = converting_share(&rolled, i);
                    probability = share + (1.0 - share) * probability;
                    if conversion_val > value {
                        value = conversion_val;
                    }
                }

                // Issuer call: exercised only when it lowers the value, given
                // the holder's best response (convert if permitted and worth
                // more than the call price, else take cash).
                if self.valuator.soft_call_triggered(node_spot)
                    && let Some(call_price) = call_price_here
                {
                    let holder_converts = can_convert && conversion_val >= call_price;
                    let value_if_called = if holder_converts {
                        conversion_val
                    } else {
                        call_price
                    };
                    if value > value_if_called {
                        value = value_if_called;
                        probability = if holder_converts { 1.0 } else { 0.0 };
                    }
                }

                // Holder put: a cash redemption.
                if let Some(put_price) = put_price_here
                    && value < put_price
                {
                    value = put_price;
                    probability = 0.0;
                }

                next_values.push((value + coupon, probability));
            }
            std::mem::swap(&mut values, &mut next_values);
        }

        Ok(values[0].0)
    }

    /// Present value of the straight cash claim: every scheduled coupon and
    /// principal payment discounted at the risk-free rate plus the full credit
    /// spread, with no conversion, call or put.
    pub(super) fn bond_floor(&self) -> Result<f64> {
        let credit_dfs = self.credit_step_dfs()?;
        let payment = |step| {
            self.valuator.coupon_map.get(&step).copied().unwrap_or(0.0)
                + self
                    .valuator
                    .principal_map
                    .get(&step)
                    .copied()
                    .unwrap_or(0.0)
        };
        let mut value = payment(self.steps);
        for step in (0..self.steps).rev() {
            value = payment(step) + value * self.valuator.rf_step_dfs[step] * credit_dfs[step];
        }
        Ok(value)
    }
}
