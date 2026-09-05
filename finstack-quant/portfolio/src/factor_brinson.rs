//! Factor-Brinson unified attribution (Jeet & Partani 2023).
//!
//! Classical Brinson-Fachler ([`crate::brinson`]) requires assets to be
//! partitioned into disjoint sectors. Factor-Brinson generalizes the same
//! allocation/selection split to *continuous* factor exposures (durations,
//! spread betas, quality scores, ...) by replacing the sector partition with
//! a factor-exposure matrix and a factor-return vector.
//!
//! # Notation
//!
//! * `h_p`, `h_b` — portfolio / benchmark asset weight vectors (length
//!   `n_assets`).
//! * `X` — asset x factor exposure matrix (`n_assets x n_factors`).
//! * `f_b` — benchmark factor returns (caller-supplied; see below).
//! * `r` — realized asset returns.
//! * `ε_b = r − X f_b` — per-asset specific return implied by `f_b`.
//!
//! # Decomposition
//!
//! ```text
//! w  = X'(h_p − h_b)          // active factor loadings
//! FC = w'f_b                   // factor contribution ("allocation")
//! SC = (h_p − h_b)'ε_b         // specific contribution ("selection")
//! FC + SC ≡ h_p'r − h_b'r = active_return   (algebraic identity, any f_b)
//! ```
//!
//! `FC + SC` reconstructs the active return for *any* `f_b` — that identity
//! is pure algebra (`w'f_b + (h_p-h_b)'ε_b = (h_p-h_b)'(Xf_b + ε_b) =
//! (h_p-h_b)'r`) and holds even for a nonsensical `f_b`. Because of this,
//! `selection` is computed *independently* (as `Σ_i (h_p-h_b)_i ε_b,i` via
//! its own accumulator) rather than as `active_return - allocation`: an
//! adversarial sweep over exposure magnitudes spanning `1e-2..1e6`,
//! adjudicated against exact rational arithmetic, found the independent
//! form's worst-case error (`-5.00e-11`) is about 6x smaller than the
//! subtraction form's (`+3.09e-10`) — both are correct to 9 significant
//! figures and diverge only at exposure magnitudes meaningless for
//! attribution, but the independent form is measurably better conditioned,
//! not just more diagnostic. What makes `FC`
//! and `SC` interpretable as Brinson-style allocation/selection effects is
//! the **completeness condition** `h_b'ε_b ≈ 0`: the benchmark's specific
//! returns implied by `f_b` must average to zero under benchmark weights,
//! i.e. `f_b` must fully explain the benchmark's realized return. Binary
//! (0/1) factor exposures forming a sector partition, with `f_b` set to
//! each sector's benchmark-weighted-mean return, satisfy this exactly and
//! reduce `FC`/`SC` to classical Brinson-Fachler allocation/(selection +
//! interaction) — see
//! `binary_factors_reproduce_brinson_fachler_exactly` in this module's
//! tests. For continuous exposures, `f_b` must come from a fit that
//! enforces the completeness condition, e.g.
//! `finstack_quant_analytics::regression::constrained_least_squares`,
//! which this crate does not depend on: factor returns are always
//! caller-supplied, and [`factor_brinson_attribution`] only checks the
//! condition, it does not fit `f_b` itself.
//!
//! # Design decisions
//!
//! * **Factor returns are caller-supplied.** They typically come from a
//!   risk model or from
//!   `finstack_quant_analytics::regression::constrained_least_squares`
//!   (this crate does not depend on `finstack-quant-analytics`: the
//!   dependency direction runs analytics -> ... -> portfolio, so adding a
//!   reverse edge purely to reference a function in doc comments and error
//!   text is unnecessary and would be an unused dependency).
//! * **Per-factor contributions are undemeaned** (`w_k * f_k`, not
//!   `w_k * (f_k - mean(f))`): demeaning is a presentation-layer choice
//!   and is ill-defined for continuous (non-partition) loadings — paper
//!   footnote 10.
//! * **Completeness tolerance.** The paper's completeness condition is
//!   `h_b'ε_b = 0` exactly for a properly fit `f_b`. In practice, `f_b`
//!   reaches this function as literal floating-point numbers — often
//!   copied from a risk-model report or rounded to a handful of
//!   significant digits — so a residual at the scale of that rounding
//!   (empirically ~1.45e-8 for 8-significant-digit inputs; see this
//!   module's `continuous_factors_match_hand_derived_estimator_output`
//!   test) is expected, not a modeling failure. [`factor_brinson_attribution`]
//!   therefore uses `COMPLETENESS_TOLERANCE` (`1e-7`) scaled by
//!   `max(1, |r_b|)` — an absolute `1e-7` for typical (sub-100%) returns,
//!   scaling up proportionally for large-magnitude `r_b` — rather than a
//!   tolerance tight enough to reject the 8-significant-digit rounding
//!   above. This bound is pinned in both directions (accept/reject just
//!   inside/outside `1e-7`, see
//!   `completeness_tolerance_is_pinned_at_1e_minus_7_both_directions`), and
//!   the `max(1, ·)` floor is independently pinned by
//!   `completeness_bound_scales_with_large_benchmark_return`, so it is not
//!   inert scaffolding — while still rejecting `f_b` values that do not
//!   explain the benchmark at all (residuals several orders of magnitude
//!   larger; see `incomplete_factor_returns_fail_closed`).
//!
//! # References
//!
//! * Jeet, V., & Partani, A. (2023). "Brinson-Style Attribution over
//!   Continuous Factors." *The Journal of Portfolio Management*,
//!   Quantitative Special Issue 2023, 216-223. `docs/REFERENCES.md#jeet-partani-2023`
//!

use crate::error::{Error, Result};
use finstack_quant_core::math::summation::NeumaierAccumulator;
use serde::{Deserialize, Serialize};

/// Tolerance for the requirement that each side's weights sum to `1.0`.
const WEIGHT_TOLERANCE: f64 = 1e-6;

/// Tolerance on the Jeet-Partani completeness residual `h_b'ε_b`, scaled by
/// `max(1, |r_b|)`. See the module-level "Completeness tolerance" note for
/// why this is `1e-7` and not a tighter or looser bound.
const COMPLETENESS_TOLERANCE: f64 = 1e-7;

/// Inputs to [`factor_brinson_attribution`]: per-asset returns, a factor
/// exposure matrix, and portfolio/benchmark weights.
///
/// Weights may be negative (short positions); each side's weights must sum
/// to `1.0` within `WEIGHT_TOLERANCE`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactorBrinsonInput {
    /// Asset identifiers, length `n_assets`.
    pub asset_ids: Vec<String>,
    /// Realized asset returns (decimal, e.g. `0.02` = 2%), length `n_assets`.
    pub asset_returns: Vec<f64>,
    /// Row-major factor exposure matrix, `n_assets x n_factors`: asset `i`'s
    /// exposure to factor `j` is `exposures[i * n_factors + j]`.
    pub exposures: Vec<f64>,
    /// Factor names, length `n_factors` (defines `n_factors`).
    pub factor_names: Vec<String>,
    /// Portfolio weight per asset, length `n_assets`. Must sum to `1.0`.
    pub portfolio_weights: Vec<f64>,
    /// Benchmark weight per asset, length `n_assets`. Must sum to `1.0`.
    pub benchmark_weights: Vec<f64>,
}

/// One factor's contribution to the factor (allocation) effect.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactorContribution {
    /// Factor name (mirrors [`FactorBrinsonInput::factor_names`]).
    pub factor: String,
    /// Active factor loading `w_k = Σ_i X_ik (h_p,i − h_b,i)`.
    pub active_loading: f64,
    /// Benchmark factor return `f_k` supplied by the caller.
    pub factor_return: f64,
    /// Contribution `w_k * f_k` (undemeaned; see module docs).
    pub contribution: f64,
}

/// One asset's contribution to the specific (selection) effect.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetSpecificContribution {
    /// Asset identifier (mirrors [`FactorBrinsonInput::asset_ids`]).
    pub asset: String,
    /// Specific return implied by the supplied factor returns,
    /// `ε_b,i = r_i − (X f_b)_i`.
    pub specific_return: f64,
    /// Active weight `h_p,i − h_b,i`.
    pub active_weight: f64,
    /// Contribution `(h_p,i − h_b,i) * ε_b,i`.
    pub contribution: f64,
}

/// Factor-Brinson unified attribution result.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactorBrinsonResult {
    /// Portfolio total return, `h_p'r`.
    pub portfolio_return: f64,
    /// Benchmark total return, `h_b'r`.
    pub benchmark_return: f64,
    /// Active return, `portfolio_return − benchmark_return`.
    pub active_return: f64,
    /// Factor (allocation) contribution, `FC = w'f_b` where
    /// `w = X'(h_p − h_b)`.
    pub allocation: f64,
    /// Specific (selection) contribution, `SC = (h_p − h_b)'ε_b`.
    pub selection: f64,
    /// Per-factor breakdown of `allocation`, in `factor_names` order.
    pub factor_contributions: Vec<FactorContribution>,
    /// Per-asset breakdown of `selection`, in `asset_ids` order.
    pub asset_contributions: Vec<AssetSpecificContribution>,
}

/// Compute Jeet-Partani (2023) factor-Brinson unified attribution.
///
/// Generalizes classical Brinson-Fachler allocation/selection to continuous
/// factor exposures. See the module docs for the full decomposition and the
/// completeness condition that makes `allocation`/`selection` interpretable
/// as Brinson-style effects.
///
/// # Arguments
///
/// * `input` - Asset returns, exposures, and portfolio/benchmark weights.
/// * `factor_returns` - Caller-supplied benchmark factor returns `f_b`,
///   length `input.factor_names.len()`. Typically fit with
///   `finstack_quant_analytics::regression::constrained_least_squares` so
///   the completeness condition holds.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] when:
/// - any of `asset_returns`, `portfolio_weights`, `benchmark_weights` does
///   not have length `n_assets = asset_ids.len()`, or `n_assets == 0`.
/// - `factor_returns.len() != input.factor_names.len()`, or
///   `input.factor_names.is_empty()` (`n_factors == 0`).
/// - `exposures.len() != n_assets * n_factors`.
/// - any input value is `NaN` or infinite.
/// - portfolio or benchmark weights do not sum to `1.0` within
///   `WEIGHT_TOLERANCE`.
/// - the completeness residual `|h_b'ε_b|` exceeds
///   `COMPLETENESS_TOLERANCE` * `max(1, |benchmark_return|)` — the
///   supplied `factor_returns` do not fully explain the benchmark return,
///   so `allocation`/`selection` would not be valid Brinson effects. The
///   error message directs callers to
///   `finstack_quant_analytics::regression::constrained_least_squares`.
pub fn factor_brinson_attribution(
    input: &FactorBrinsonInput,
    factor_returns: &[f64],
) -> Result<FactorBrinsonResult> {
    let n_assets = input.asset_ids.len();
    let n_factors = input.factor_names.len();

    if n_assets == 0 {
        return Err(Error::invalid_input(
            "factor-Brinson attribution requires at least one asset",
        ));
    }
    if n_factors == 0 {
        return Err(Error::invalid_input(
            "factor-Brinson attribution requires at least one factor",
        ));
    }
    for (name, len) in [
        ("asset_returns", input.asset_returns.len()),
        ("portfolio_weights", input.portfolio_weights.len()),
        ("benchmark_weights", input.benchmark_weights.len()),
    ] {
        if len != n_assets {
            return Err(Error::invalid_input(format!(
                "'{name}' has length {len}, expected {n_assets} (= asset_ids.len())"
            )));
        }
    }
    if factor_returns.len() != n_factors {
        return Err(Error::invalid_input(format!(
            "'factor_returns' has length {}, expected {n_factors} (= factor_names.len())",
            factor_returns.len()
        )));
    }
    if input.exposures.len() != n_assets * n_factors {
        return Err(Error::invalid_input(format!(
            "'exposures' has length {}, expected {} (= n_assets * n_factors)",
            input.exposures.len(),
            n_assets * n_factors
        )));
    }

    for (i, &v) in input.asset_returns.iter().enumerate() {
        if !v.is_finite() {
            return Err(Error::invalid_input(format!(
                "'asset_returns[{i}]' must be finite (got {v})"
            )));
        }
    }
    for (i, &v) in input.portfolio_weights.iter().enumerate() {
        if !v.is_finite() {
            return Err(Error::invalid_input(format!(
                "'portfolio_weights[{i}]' must be finite (got {v})"
            )));
        }
    }
    for (i, &v) in input.benchmark_weights.iter().enumerate() {
        if !v.is_finite() {
            return Err(Error::invalid_input(format!(
                "'benchmark_weights[{i}]' must be finite (got {v})"
            )));
        }
    }
    for (i, &v) in input.exposures.iter().enumerate() {
        if !v.is_finite() {
            return Err(Error::invalid_input(format!(
                "'exposures[{i}]' must be finite (got {v})"
            )));
        }
    }
    for (j, &v) in factor_returns.iter().enumerate() {
        if !v.is_finite() {
            return Err(Error::invalid_input(format!(
                "'factor_returns[{j}]' must be finite (got {v})"
            )));
        }
    }

    let sum_wp: f64 = {
        let mut acc = NeumaierAccumulator::new();
        for &w in &input.portfolio_weights {
            acc.add(w);
        }
        acc.total()
    };
    if (sum_wp - 1.0).abs() > WEIGHT_TOLERANCE {
        return Err(Error::invalid_input(format!(
            "Portfolio weights must sum to 1.0 (got {sum_wp})"
        )));
    }
    let sum_wb: f64 = {
        let mut acc = NeumaierAccumulator::new();
        for &w in &input.benchmark_weights {
            acc.add(w);
        }
        acc.total()
    };
    if (sum_wb - 1.0).abs() > WEIGHT_TOLERANCE {
        return Err(Error::invalid_input(format!(
            "Benchmark weights must sum to 1.0 (got {sum_wb})"
        )));
    }

    // Portfolio / benchmark total returns: r_p = h_p'r, r_b = h_b'r.
    let mut acc_rp = NeumaierAccumulator::new();
    let mut acc_rb = NeumaierAccumulator::new();
    for ((&hp_i, &hb_i), &r_i) in input
        .portfolio_weights
        .iter()
        .zip(input.benchmark_weights.iter())
        .zip(input.asset_returns.iter())
    {
        acc_rp.add(hp_i * r_i);
        acc_rb.add(hb_i * r_i);
    }
    let portfolio_return = acc_rp.total();
    let benchmark_return = acc_rb.total();
    let active_return = portfolio_return - benchmark_return;

    // Active weight h_p - h_b, per asset.
    let active_weight: Vec<f64> = (0..n_assets)
        .map(|i| input.portfolio_weights[i] - input.benchmark_weights[i])
        .collect();

    // Active factor loadings w = X'(h_p - h_b): one Neumaier accumulator
    // per factor, summed over assets in input order.
    let mut acc_w: Vec<NeumaierAccumulator> = vec![NeumaierAccumulator::new(); n_factors];
    for (row, &aw_i) in input
        .exposures
        .chunks_exact(n_factors)
        .zip(active_weight.iter())
    {
        for (acc, &x_ij) in acc_w.iter_mut().zip(row.iter()) {
            acc.add(x_ij * aw_i);
        }
    }
    let w: Vec<f64> = acc_w.into_iter().map(NeumaierAccumulator::total).collect();

    // Factor (allocation) contribution FC = w'f_b.
    let mut acc_fc = NeumaierAccumulator::new();
    for (&w_j, &f_j) in w.iter().zip(factor_returns.iter()) {
        acc_fc.add(w_j * f_j);
    }
    let allocation = acc_fc.total();

    // Per-asset specific returns implied by f_b: eps_b,i = r_i - (X f_b)_i.
    let specific_return: Vec<f64> = (0..n_assets)
        .map(|i| {
            let row = &input.exposures[i * n_factors..(i + 1) * n_factors];
            let mut acc_xf = NeumaierAccumulator::new();
            for (j, &x_ij) in row.iter().enumerate() {
                acc_xf.add(x_ij * factor_returns[j]);
            }
            input.asset_returns[i] - acc_xf.total()
        })
        .collect();

    // Completeness condition: h_b'eps_b must be ~0 for f_b to be a valid
    // benchmark-return-explaining fit (see module docs).
    let mut acc_completeness = NeumaierAccumulator::new();
    for (&hb_i, &eps_i) in input.benchmark_weights.iter().zip(specific_return.iter()) {
        acc_completeness.add(hb_i * eps_i);
    }
    let completeness_residual = acc_completeness.total();
    let completeness_bound = COMPLETENESS_TOLERANCE * benchmark_return.abs().max(1.0);
    if completeness_residual.abs() > completeness_bound {
        return Err(Error::invalid_input(format!(
            "Supplied factor_returns do not satisfy the Jeet-Partani completeness \
             condition: |h_b'eps_b| = {} exceeds tolerance {completeness_bound} \
             (benchmark_return = {benchmark_return}). allocation/selection would not \
             be valid Brinson effects for these factor returns. Fit factor_returns \
             with finstack_quant_analytics::regression::constrained_least_squares \
             (using benchmark weights) to enforce this condition.",
            completeness_residual.abs()
        )));
    }

    // Specific (selection) contribution SC = (h_p - h_b)'eps_b, computed
    // independently of `allocation` and `active_return` (not by
    // subtraction) so this check retains diagnostic value against a
    // mis-derived selection term. This is also measurably better
    // conditioned than the subtraction form (see module docs).
    let mut acc_sc = NeumaierAccumulator::new();
    for (&aw_i, &eps_i) in active_weight.iter().zip(specific_return.iter()) {
        acc_sc.add(aw_i * eps_i);
    }
    let selection = acc_sc.total();

    let factor_contributions = (0..n_factors)
        .map(|j| FactorContribution {
            factor: input.factor_names[j].clone(),
            active_loading: w[j],
            factor_return: factor_returns[j],
            contribution: w[j] * factor_returns[j],
        })
        .collect();

    let asset_contributions = (0..n_assets)
        .map(|i| AssetSpecificContribution {
            asset: input.asset_ids[i].clone(),
            specific_return: specific_return[i],
            active_weight: active_weight[i],
            contribution: active_weight[i] * specific_return[i],
        })
        .collect();

    Ok(FactorBrinsonResult {
        portfolio_return,
        benchmark_return,
        active_return,
        allocation,
        selection,
        factor_contributions,
        asset_contributions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brinson::{brinson_fachler, SectorPeriod};

    fn close(actual: f64, expected: f64, tol: f64, label: &str) {
        assert!(
            (actual - expected).abs() < tol,
            "{label}: {actual} vs {expected} (tol {tol})"
        );
    }

    fn base_input(exposures: Vec<f64>) -> FactorBrinsonInput {
        FactorBrinsonInput {
            asset_ids: vec!["A".into(), "B".into(), "C".into()],
            asset_returns: vec![0.05, 0.02, 0.01],
            exposures,
            factor_names: vec!["Energy".into(), "Healthcare".into()],
            portfolio_weights: vec![1.25, -0.30, 0.05],
            benchmark_weights: vec![0.60, 0.30, 0.10],
        }
    }

    /// Paper Exhibits 1-2 in decimals, binary (0/1) sector-indicator
    /// exposures. `f_b` is each sector's benchmark-weighted-mean return:
    /// Energy 0.02 (sole asset B), Healthcare 0.31/7 = 0.0442857142857
    /// (assets A, C weighted 0.60/0.10 in the benchmark).
    #[test]
    fn binary_factors_reproduce_brinson_fachler_exactly() {
        // Rows: A -> Healthcare only, B -> Energy only, C -> Healthcare only.
        let input = base_input(vec![0.0, 1.0, 1.0, 0.0, 0.0, 1.0]);
        let f_b = [0.02, 0.31 / 7.0];
        let r = factor_brinson_attribution(&input, &f_b).expect("valid inputs");

        close(r.active_return, 0.02, 1e-12, "active_return");
        close(r.allocation, 0.0145714285714286, 1e-12, "allocation"); // paper: 1.46%
        close(r.selection, 0.0054285714285714, 1e-12, "selection"); // paper: 0.54%

        // Equivalence with the existing Brinson-Fachler implementation.
        //
        // Energy = {B}, Healthcare = {A, C}. Healthcare's portfolio sector
        // return is the portfolio-weighted mean of A and C. Written as the
        // exact expression (not a truncated decimal literal) so the fixture
        // doesn't inject its own rounding error against the 1e-12 tolerance
        // below.
        let sectors = vec![
            SectorPeriod {
                sector: "Energy".into(),
                portfolio_weight: -0.30,
                benchmark_weight: 0.30,
                portfolio_return: 0.02,
                benchmark_return: 0.02,
            },
            SectorPeriod {
                sector: "Healthcare".into(),
                portfolio_weight: 1.30,
                benchmark_weight: 0.70,
                portfolio_return: (1.25 * 0.05 + 0.05 * 0.01) / 1.30,
                benchmark_return: 0.31 / 7.0,
            },
        ];
        let bf = brinson_fachler(&sectors).expect("valid BF inputs");

        close(bf.total_allocation, r.allocation, 1e-12, "BF allocation");
        // This is the one place classical BF and factor-Brinson
        // terminology diverge. Classical BF's `selection` term is
        // benchmark-weighted (`w_b,i (r_p,i - r_b,i)`) and reports the
        // *joint* effect of over/underweighting and stock-picking
        // separately as `interaction` (`(w_p,i - w_b,i)(r_p,i - r_b,i)`).
        // Factor-Brinson's specific contribution `SC = (h_p - h_b)'eps_b`
        // carries the *active* weight `h_p - h_b`, which is exactly BF's
        // `selection + interaction` combined:
        //   BF selection    = w_b,Healthcare * (r_p - r_b)
        //                    = 0.70 * 0.0041758241758 = 0.0029230769231
        //   BF interaction  = (w_p,Healthcare - w_b,Healthcare) * (r_p - r_b)
        //                    = 0.60 * 0.0041758241758 = 0.0025054945055
        //   sum             = 0.0054285714286 = r.selection
        close(
            bf.total_selection + bf.total_interaction,
            r.selection,
            1e-12,
            "BF selection + interaction",
        );
    }

    /// Pins every field of every element of `factor_contributions` and
    /// `asset_contributions` for the binary fixture, in input order. The
    /// two top-level scalars (`allocation`, `selection`) reconciling with
    /// their breakdown sums is necessary but not sufficient: a presentation
    /// sort on either `Vec`, an off-by-one in the per-factor/per-asset
    /// loop, or a zeroed individual field can all leave the top-level sums
    /// unchanged. Exact values independently derived with exact rational
    /// arithmetic (see task report), not back-computed from the code under
    /// test.
    #[test]
    fn factor_and_asset_contributions_are_pinned_exactly() {
        let input = base_input(vec![0.0, 1.0, 1.0, 0.0, 0.0, 1.0]);
        let f_b = [0.02, 0.31 / 7.0];
        let r = factor_brinson_attribution(&input, &f_b).expect("valid inputs");

        assert_eq!(r.factor_contributions.len(), 2, "factor_contributions len");
        let fc = &r.factor_contributions[0];
        assert_eq!(fc.factor, "Energy", "factor_contributions[0].factor");
        close(fc.active_loading, -0.60, 1e-12, "fc[0].active_loading");
        close(fc.factor_return, 0.02, 1e-12, "fc[0].factor_return");
        close(fc.contribution, -0.012, 1e-12, "fc[0].contribution");

        let fc = &r.factor_contributions[1];
        assert_eq!(fc.factor, "Healthcare", "factor_contributions[1].factor");
        close(fc.active_loading, 0.60, 1e-12, "fc[1].active_loading");
        close(fc.factor_return, 0.31 / 7.0, 1e-12, "fc[1].factor_return");
        close(
            fc.contribution,
            0.0265714285714286,
            1e-12,
            "fc[1].contribution",
        );

        assert_eq!(r.asset_contributions.len(), 3, "asset_contributions len");
        let ac = &r.asset_contributions[0];
        assert_eq!(ac.asset, "A", "asset_contributions[0].asset");
        close(
            ac.specific_return,
            0.0057142857142857,
            1e-12,
            "ac[0].specific_return",
        );
        close(ac.active_weight, 0.65, 1e-12, "ac[0].active_weight");
        close(
            ac.contribution,
            0.0037142857142857,
            1e-12,
            "ac[0].contribution",
        );

        let ac = &r.asset_contributions[1];
        assert_eq!(ac.asset, "B", "asset_contributions[1].asset");
        close(ac.specific_return, 0.0, 1e-12, "ac[1].specific_return");
        close(ac.active_weight, -0.60, 1e-12, "ac[1].active_weight");
        close(ac.contribution, 0.0, 1e-12, "ac[1].contribution");

        let ac = &r.asset_contributions[2];
        assert_eq!(ac.asset, "C", "asset_contributions[2].asset");
        close(
            ac.specific_return,
            -0.0342857142857143,
            1e-12,
            "ac[2].specific_return",
        );
        close(ac.active_weight, -0.05, 1e-12, "ac[2].active_weight");
        close(
            ac.contribution,
            0.0017142857142857,
            1e-12,
            "ac[2].contribution",
        );

        let fc_sum: f64 = r.factor_contributions.iter().map(|c| c.contribution).sum();
        close(
            fc_sum,
            r.allocation,
            1e-12,
            "sum(factor_contributions.contribution) == allocation",
        );
        let ac_sum: f64 = r.asset_contributions.iter().map(|c| c.contribution).sum();
        close(
            ac_sum,
            r.selection,
            1e-12,
            "sum(asset_contributions.contribution) == selection",
        );
    }

    /// Exposures from paper Exhibit 6 (continuous, not a sector partition);
    /// `f_b` from Task 6's constrained-least-squares estimator (hand
    /// values, rounded to 8 significant digits as a caller would report
    /// them).
    #[test]
    fn continuous_factors_match_hand_derived_estimator_output() {
        let input = base_input(vec![1.2, -0.8, 0.5, 1.2, -0.7, 0.7]);
        let f_b = [0.04695653, 0.01130477];
        let r = factor_brinson_attribution(&input, &f_b).expect("valid inputs");

        close(r.allocation, 0.0097690, 1e-6, "allocation");
        close(r.selection, 0.0102310, 1e-6, "selection");
        // FC + SC = active_return is an algebraic identity for any f_b
        // (see module docs) — it does not by itself validate `selection`,
        // only that the two terms were computed consistently.
        close(
            r.allocation + r.selection,
            r.active_return,
            1e-14,
            "FC + SC = active_return",
        );
    }

    /// An arbitrary `f_b` that does not explain the benchmark's realized
    /// return must be rejected: `h_b'eps_b` is far outside tolerance.
    #[test]
    fn incomplete_factor_returns_fail_closed() {
        let input = base_input(vec![1.2, -0.8, 0.5, 1.2, -0.7, 0.7]);
        let f_b = [0.05, 0.01];
        let err = factor_brinson_attribution(&input, &f_b)
            .expect_err("f_b must fail the completeness condition");
        assert!(
            err.to_string().contains("constrained_least_squares"),
            "error should direct callers to the fitting function: {err}"
        );
    }

    /// Pins `COMPLETENESS_TOLERANCE` (`1e-7`) itself, in both directions,
    /// mirroring the `WEIGHT_TOLERANCE` both-directions pins below.
    /// Perturbing `f_b[0]` by `delta` changes the completeness residual by
    /// `-delta * Σ_i h_b,i X_i0` (since `h_b'ε_b = benchmark_return -
    /// (Σ h_b X)'f_b`); for this fixture `Σ h_b X = (0.8, -0.05)`, so
    /// `delta = 1.0e-7` keeps the residual just inside the bound and
    /// `delta = 2.5e-7` (the reviewer's suggested `2e-7 / |Σ h_b X|`) pushes
    /// it just outside.
    #[test]
    fn completeness_tolerance_is_pinned_at_1e_minus_7_both_directions() {
        let input = base_input(vec![1.2, -0.8, 0.5, 1.2, -0.7, 0.7]);
        let f_b_base = [0.04695653, 0.01130477];

        let mut just_inside = f_b_base;
        just_inside[0] += 1.0e-7;
        assert!(
            factor_brinson_attribution(&input, &just_inside).is_ok(),
            "a completeness residual just inside 1e-7 must be accepted"
        );

        let mut just_outside = f_b_base;
        just_outside[0] += 2.5e-7;
        let err = factor_brinson_attribution(&input, &just_outside)
            .expect_err("a completeness residual just outside 1e-7 must be rejected");
        assert!(err.to_string().contains("completeness"), "{err}");
    }

    /// Pins the `max(1, |r_b|)` floor in the completeness bound: without it
    /// (or if it were replaced by a hardcoded `1.0`), a large-magnitude
    /// benchmark return would use a bound far tighter than is reasonable
    /// for a proportionally large residual, and this fixture's residual
    /// would be wrongly rejected. `n_factors = 1`, uniform unit exposure, so
    /// `eps_b,i = r_i - f_b` and `h_b'eps_b = benchmark_return - f_b`
    /// exactly: `benchmark_return = 5.0`, `f_b = 5.0 - 3e-7` gives a
    /// residual of exactly `3e-7`, which must be accepted only because the
    /// bound scales up to `1e-7 * 5.0 = 5e-7`.
    #[test]
    fn completeness_bound_scales_with_large_benchmark_return() {
        let input = FactorBrinsonInput {
            asset_ids: vec!["A".into(), "B".into()],
            asset_returns: vec![6.0, 4.0],
            exposures: vec![1.0, 1.0],
            factor_names: vec!["F".into()],
            portfolio_weights: vec![0.5, 0.5],
            benchmark_weights: vec![0.5, 0.5],
        };
        let f_b = [5.0 - 3e-7];
        let r = factor_brinson_attribution(&input, &f_b).expect(
            "residual 3e-7 must be accepted: bound = 1e-7 * max(1, 5.0) = 5e-7, not 1e-7 * 1",
        );
        close(r.benchmark_return, 5.0, 1e-12, "benchmark_return");
    }

    #[test]
    fn dimension_and_weight_validation() {
        let input = base_input(vec![1.2, -0.8, 0.5, 1.2, -0.7, 0.7]);
        let f_b = [0.04695653, 0.01130477];

        // Mismatched asset_returns length.
        let mut bad = input.clone();
        bad.asset_returns = vec![0.05, 0.02];
        let err = factor_brinson_attribution(&bad, &f_b).expect_err("length mismatch");
        assert!(err.to_string().contains("asset_returns"), "{err}");

        // Mismatched exposures length.
        let mut bad = input.clone();
        bad.exposures = vec![1.0, 2.0];
        let err = factor_brinson_attribution(&bad, &f_b).expect_err("exposures mismatch");
        assert!(err.to_string().contains("exposures"), "{err}");

        // Mismatched factor_returns length.
        let err = factor_brinson_attribution(&input, &[0.05])
            .expect_err("factor_returns length mismatch");
        assert!(err.to_string().contains("factor_returns"), "{err}");

        // n_factors == 0.
        let mut bad = input.clone();
        bad.factor_names = vec![];
        bad.exposures = vec![];
        let err = factor_brinson_attribution(&bad, &[]).expect_err("zero factors");
        assert!(err.to_string().contains("at least one factor"), "{err}");

        // n_assets == 0.
        let empty = FactorBrinsonInput {
            asset_ids: vec![],
            asset_returns: vec![],
            exposures: vec![],
            factor_names: vec!["F".into()],
            portfolio_weights: vec![],
            benchmark_weights: vec![],
        };
        let err = factor_brinson_attribution(&empty, &[0.01]).expect_err("zero assets");
        assert!(err.to_string().contains("asset"), "{err}");

        // NaN asset return.
        let mut bad = input.clone();
        bad.asset_returns[0] = f64::NAN;
        let err = factor_brinson_attribution(&bad, &f_b).expect_err("NaN return");
        assert!(err.to_string().contains("finite"), "{err}");

        // NaN exposure.
        let mut bad = input.clone();
        bad.exposures[0] = f64::INFINITY;
        let err = factor_brinson_attribution(&bad, &f_b).expect_err("infinite exposure");
        assert!(err.to_string().contains("finite"), "{err}");

        // Portfolio weights sum to 1 + 2e-6: rejected.
        let mut bad = input.clone();
        bad.portfolio_weights[0] += 2e-6;
        let err = factor_brinson_attribution(&bad, &f_b).expect_err("1 + 2e-6 must be rejected");
        assert!(err.to_string().contains("Portfolio weights"), "{err}");

        // Portfolio weights sum to 1 + 5e-7: accepted.
        let mut ok = input.clone();
        ok.portfolio_weights[0] += 5e-7;
        assert!(
            factor_brinson_attribution(&ok, &f_b).is_ok(),
            "1 + 5e-7 must be accepted at a 1e-6 tolerance"
        );

        // Benchmark weights sum to 1 + 2e-6: rejected.
        let mut bad = input.clone();
        bad.benchmark_weights[0] += 2e-6;
        let err = factor_brinson_attribution(&bad, &f_b)
            .expect_err("benchmark 1 + 2e-6 must be rejected");
        assert!(err.to_string().contains("Benchmark weights"), "{err}");

        // Benchmark weights sum to 1 + 5e-7: accepted.
        let mut ok = input;
        ok.benchmark_weights[0] += 5e-7;
        assert!(
            factor_brinson_attribution(&ok, &f_b).is_ok(),
            "benchmark 1 + 5e-7 must be accepted at a 1e-6 tolerance"
        );
    }

    /// Negative (short) weights must be supported on the *benchmark* side
    /// too, not just the portfolio side (every other fixture in this module
    /// has an all-positive `benchmark_weights`). Energy = {B}, Healthcare =
    /// {A, C} as in the binary golden, but the benchmark is short 20% of B:
    /// `benchmark_weights = (0.9, -0.2, 0.3)`. Values hand-derived
    /// independently (not back-computed from the code under test):
    ///
    /// `f_Energy = r_B = 0.02` (single-asset sector, weight sign is
    /// irrelevant to a single asset's own return); `f_Healthcare =
    /// (0.9*0.05 + 0.3*0.01) / (0.9+0.3) = 0.04`. `eps_b = (0.01, 0, -0.03)`,
    /// and `h_b'eps_b = 0.9*0.01 - 0.2*0 + 0.3*(-0.03) = 0` exactly, so the
    /// completeness condition still holds with a benchmark short.
    ///
    /// `active_weight = h_p - h_b = (0.35, -0.10, -0.25)`,
    /// `w = (active_weight_B, active_weight_A + active_weight_C) =
    /// (-0.10, 0.10)`, `allocation = -0.10*0.02 + 0.10*0.04 = 0.002`,
    /// `selection = 0.35*0.01 - 0.10*0 - 0.25*(-0.03) = 0.011`,
    /// `active_return = portfolio_return - benchmark_return = 0.057 -
    /// 0.044 = 0.013 = allocation + selection`.
    #[test]
    fn benchmark_short_position_is_supported() {
        let input = FactorBrinsonInput {
            asset_ids: vec!["A".into(), "B".into(), "C".into()],
            asset_returns: vec![0.05, 0.02, 0.01],
            exposures: vec![0.0, 1.0, 1.0, 0.0, 0.0, 1.0],
            factor_names: vec!["Energy".into(), "Healthcare".into()],
            portfolio_weights: vec![1.25, -0.30, 0.05],
            benchmark_weights: vec![0.9, -0.2, 0.3],
        };
        assert!(input.benchmark_weights.iter().any(|&w| w < 0.0));
        let f_b = [0.02, 0.04];
        let r = factor_brinson_attribution(&input, &f_b).expect("valid inputs");

        close(r.portfolio_return, 0.057, 1e-12, "portfolio_return");
        close(r.benchmark_return, 0.044, 1e-12, "benchmark_return");
        close(r.active_return, 0.013, 1e-12, "active_return");
        close(r.allocation, 0.002, 1e-12, "allocation");
        close(r.selection, 0.011, 1e-12, "selection");
        close(
            r.allocation + r.selection,
            r.active_return,
            1e-14,
            "FC + SC = active_return",
        );
    }

    #[test]
    fn factor_brinson_result_serde_round_trips() {
        let input = base_input(vec![0.0, 1.0, 1.0, 0.0, 0.0, 1.0]);
        let f_b = [0.02, 0.31 / 7.0];
        let r = factor_brinson_attribution(&input, &f_b).expect("valid inputs");

        let json = serde_json::to_string(&r).expect("serializes");
        let round_tripped: FactorBrinsonResult = serde_json::from_str(&json).expect("deserializes");

        close(
            round_tripped.portfolio_return,
            r.portfolio_return,
            1e-15,
            "portfolio_return",
        );
        close(
            round_tripped.benchmark_return,
            r.benchmark_return,
            1e-15,
            "benchmark_return",
        );
        close(
            round_tripped.active_return,
            r.active_return,
            1e-15,
            "active_return",
        );
        close(round_tripped.allocation, r.allocation, 1e-15, "allocation");
        close(round_tripped.selection, r.selection, 1e-15, "selection");
        assert_eq!(
            round_tripped.factor_contributions.len(),
            r.factor_contributions.len()
        );
        assert_eq!(
            round_tripped.asset_contributions.len(),
            r.asset_contributions.len()
        );
        assert_eq!(
            round_tripped.factor_contributions[0].factor,
            r.factor_contributions[0].factor
        );
    }

    #[test]
    fn factor_brinson_input_serde_denies_unknown_fields() {
        let json = r#"{
            "asset_ids": ["A"],
            "asset_returns": [0.05],
            "exposures": [1.0],
            "factor_names": ["F"],
            "portfolio_weights": [1.0],
            "benchmark_weights": [1.0]
        }"#;
        let parsed: FactorBrinsonInput = serde_json::from_str(json).expect("stable names parse");
        assert_eq!(parsed.asset_ids[0], "A");

        let bad = r#"{
            "asset_ids": ["A"],
            "asset_returns": [0.05],
            "exposures": [1.0],
            "factor_names": ["F"],
            "portfolio_weights": [1.0],
            "benchmark_weights": [1.0],
            "surprise": 1.0
        }"#;
        assert!(
            serde_json::from_str::<FactorBrinsonInput>(bad).is_err(),
            "unknown fields must be rejected"
        );
    }
}
