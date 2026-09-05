//! Position-level VaR and ES decomposition via Euler allocation.
//!
//! This module provides parametric and historical decomposition of portfolio
//! VaR and Expected Shortfall into per-position contributions. The parametric
//! engine uses covariance-based Euler allocation (exact under normality); the
//! historical engine attributes tail losses from scenario P&L matrices.
//!
//! # Euler Decomposition Property
//!
//! Under the parametric (normal) assumption:
//! ```text
//! sum(component_var_i) == portfolio_var  (exact)
//! sum(component_es_i)  == portfolio_es   (exact)
//! ```
//!
//! Under historical simulation the Euler property holds approximately.
//!
//! # References
//!
//! - `docs/REFERENCES.md#tasche-2008-capital-allocation`
//! - `docs/REFERENCES.md#meucci-risk-and-asset-allocation`
//! - `docs/REFERENCES.md#litterman-1996-hotspots`

use serde::{Deserialize, Serialize};
use tracing::warn;

/// Method used for position-level VaR/ES decomposition.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecompositionMethod {
    /// Covariance-based using normal distribution assumption.
    ///
    /// Fast (O(n^2) in positions). Exact Euler property.
    /// Requires a position-level return covariance matrix.
    Parametric,

    /// Full historical simulation with per-position P&L attribution.
    ///
    /// Slow (O(n * scenarios)). Approximate Euler property.
    /// Handles non-normality and non-linear positions.
    Historical,
}

/// Configuration for position-level VaR decomposition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecompositionConfig {
    /// Confidence level for VaR and ES (e.g. 0.95, 0.99).
    pub confidence: f64,

    /// Decomposition method.
    pub method: DecompositionMethod,

    /// Whether to compute incremental VaR (expensive: one full repricing
    /// per position).
    pub compute_incremental: bool,
}

impl DecompositionConfig {
    /// Parametric configuration at an arbitrary confidence level.
    ///
    /// Binding entry points (`parametric_var_decomposition` /
    /// `parametric_es_decomposition`) accept the confidence directly rather than
    /// mutating a preset.
    ///
    /// # Arguments
    ///
    /// * `confidence` - Tail confidence as a decimal probability (e.g. `0.95`).
    pub fn parametric(confidence: f64) -> Self {
        Self {
            confidence,
            method: DecompositionMethod::Parametric,
            compute_incremental: false,
        }
    }

    /// Standard 95% parametric configuration.
    pub fn parametric_95() -> Self {
        Self::parametric(0.95)
    }

    /// Standard 99% parametric configuration.
    pub fn parametric_99() -> Self {
        Self::parametric(0.99)
    }

    /// Historical simulation configuration.
    pub fn historical(confidence: f64) -> Self {
        Self {
            confidence,
            method: DecompositionMethod::Historical,
            compute_incremental: false,
        }
    }

    /// Enable incremental VaR computation.
    #[must_use]
    pub fn with_incremental(mut self) -> Self {
        self.compute_incremental = true;
        self
    }
}

// Per-position result structs

/// Risk decomposition result for a single portfolio position.
///
/// All monetary fields are in the same units as the portfolio VaR
/// (typically the portfolio's base currency).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PositionVarContribution {
    /// Position identifier.
    pub position_id: String,

    /// Component VaR: position's Euler-allocated share of portfolio VaR.
    ///
    /// Sum of all component VaRs equals total portfolio VaR (exact under
    /// the parametric normal assumption; approximate for historical).
    ///
    /// Formula (parametric, loss-signed):
    /// ```text
    /// CVaR_i = -w_i * (Sigma * w)_i / sigma_p * z_alpha
    /// ```
    pub component_var: f64,

    /// Component VaR as a fraction of total portfolio VaR.
    ///
    /// `relative_var = component_var / portfolio_var`. Sums to 1.0.
    /// A negative value indicates the position is a diversifier.
    pub relative_var: f64,

    /// Marginal VaR: per-unit sensitivity of portfolio VaR to this position.
    ///
    /// Formula (parametric, loss-signed):
    /// ```text
    /// MVaR_i = -(Sigma * w)_i / sigma_p * z_alpha
    /// ```
    ///
    /// Used as gradient input for mean-variance optimization and
    /// risk-budgeting rebalancing.
    ///
    /// `None` when the engine cannot produce a true gradient (e.g.
    /// historical mode without finite-difference repricing); callers that
    /// need a marginal must choose a fallback or skip rebalancing.
    pub marginal_var: Option<f64>,

    /// Incremental VaR: change in portfolio VaR from removing this position.
    ///
    /// ```text
    /// IVaR_i = VaR(portfolio) - VaR(portfolio \ {i})
    /// ```
    ///
    /// Requires full repricing for each position removal. `None` if
    /// incremental VaR was not requested (it is expensive).
    pub incremental_var: Option<f64>,
}

/// Expected Shortfall decomposition result for a single portfolio position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PositionEsContribution {
    /// Position identifier.
    pub position_id: String,

    /// Component ES: position's contribution to portfolio Expected Shortfall.
    ///
    /// Parametric: analytical formula using truncated normal moments.
    /// ```text
    /// CES_i = w_i * (Sigma * w)_i / sigma_p * phi(z_alpha) / (1 - alpha)
    /// ```
    ///
    /// Historical: average of position-level losses in tail scenarios.
    /// ```text
    /// CES_i = E[L_i | L_portfolio > VaR_portfolio]
    /// ```
    pub component_es: f64,

    /// Component ES as a fraction of total portfolio ES.
    pub relative_es: f64,

    /// Marginal ES: per-unit sensitivity of portfolio ES to this position.
    ///
    /// `None` when the engine cannot produce a true gradient (e.g.
    /// historical mode without finite-difference repricing).
    pub marginal_es: Option<f64>,
}

// Aggregate result

/// Complete position-level risk decomposition of a portfolio.
///
/// Contains VaR and ES decompositions for every position, along with
/// portfolio-level totals. All values are in the portfolio's base currency.
///
/// # Euler Decomposition Property
///
/// Under the parametric (normal) assumption:
/// ```text
/// sum(component_var_i) == portfolio_var  (exact)
/// sum(component_es_i)  == portfolio_es   (exact)
/// ```
///
/// Under historical simulation, the Euler property holds approximately.
///
/// # References
///
/// - Tasche (2008): Capital allocation with Euler's method. `docs/REFERENCES.md#tasche-2008-capital-allocation`
/// - Meucci (2005): Risk and Asset Allocation. `docs/REFERENCES.md#meucci-risk-and-asset-allocation`
/// - Litterman (1996): Hot Spots and Hedges. `docs/REFERENCES.md#litterman-1996-hotspots`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PositionRiskDecomposition {
    /// Total portfolio VaR.
    ///
    /// Loss convention (workspace-wide): VaR follows the P&L sign, so
    /// losses are reported as **negative** numbers, matching the
    /// factor-level engines and `analytics::value_at_risk`.
    pub portfolio_var: f64,

    /// Total portfolio Expected Shortfall.
    ///
    /// Same loss convention as [`Self::portfolio_var`]; ES lies at or
    /// beyond VaR in the loss tail (`portfolio_es <= portfolio_var`).
    pub portfolio_es: f64,

    /// Confidence level used for both VaR and ES.
    pub confidence: f64,

    /// Method used for decomposition.
    pub method: DecompositionMethod,

    /// Per-position VaR decomposition.
    pub var_contributions: Vec<PositionVarContribution>,

    /// Per-position ES decomposition.
    pub es_contributions: Vec<PositionEsContribution>,

    /// Number of positions in the portfolio.
    pub n_positions: usize,

    /// Residual from Euler decomposition (should be near zero).
    ///
    /// `residual = portfolio_var - sum(component_var_i)`.
    /// Only meaningful for the parametric engine, where a non-zero residual
    /// signals a numerical issue (ill-conditioned covariance, floating-point
    /// accumulation error).
    ///
    /// `None` in historical mode: the Tasche scaling used there makes the
    /// residual algebraically zero by construction, so it carries no
    /// diagnostic information.
    pub euler_residual: Option<f64>,
}

impl PositionRiskDecomposition {
    /// Look up one position's component VaR by identifier.
    ///
    /// # Arguments
    ///
    /// * `position_id` - Position identifier exactly as it appears in
    ///   [`PositionVarContribution::position_id`].
    ///
    /// # Returns
    ///
    /// The position's Euler-allocated component VaR (loss convention, same
    /// units as [`Self::portfolio_var`]), or `None` when the id is absent.
    #[must_use]
    pub fn component_var(&self, position_id: &str) -> Option<f64> {
        self.var_contributions
            .iter()
            .find(|c| c.position_id == position_id)
            .map(|c| c.component_var)
    }
}

// Stress attribution (historical)

/// Per-position attribution of portfolio losses in tail scenarios.
///
/// For each scenario that breaches the VaR threshold, reports which
/// positions contributed the most to the portfolio loss.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressAttribution {
    /// Portfolio VaR threshold (scenarios with P&L at or below this are
    /// "tail events"). Loss convention: reported as a negative number.
    pub var_threshold: f64,

    /// Number of tail scenarios analyzed.
    pub n_tail_scenarios: usize,

    /// Canonical position ordering shared by every [`tail_scenarios`] entry.
    ///
    /// `tail_scenarios[k].position_pnls[i]` is the P&L for `position_ids[i]`.
    /// Storing the id list once here (rather than re-attaching it to every tail
    /// scenario) avoids duplicating it `n_tail_scenarios` times.
    ///
    /// [`tail_scenarios`]: Self::tail_scenarios
    pub position_ids: Vec<String>,

    /// Per-position average contribution to tail losses.
    ///
    /// Sorted by absolute contribution (largest risk driver first).
    pub position_contributions: Vec<StressPositionEntry>,

    /// Individual tail scenario breakdowns.
    ///
    /// Contains `n_tail_scenarios` entries, each with per-position P&L
    /// index-aligned to [`position_ids`](Self::position_ids).
    /// Sorted by portfolio loss (worst first).
    pub tail_scenarios: Vec<TailScenarioBreakdown>,
}

/// Single position's contribution to tail stress.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressPositionEntry {
    /// Position identifier.
    pub position_id: String,

    /// Average P&L contribution in tail scenarios.
    pub avg_tail_pnl: f64,

    /// Fraction of total portfolio tail loss attributable to this position.
    pub pct_of_tail_loss: f64,

    /// Worst single-scenario P&L for this position.
    pub worst_scenario_pnl: f64,
}

/// Breakdown of a single tail scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TailScenarioBreakdown {
    /// Scenario index in the original history.
    pub scenario_index: usize,

    /// Total portfolio P&L for this scenario.
    pub portfolio_pnl: f64,

    /// Per-position P&L contributions, index-aligned to
    /// [`StressAttribution::position_ids`]. Entry `i` is the P&L for
    /// `StressAttribution::position_ids[i]`.
    pub position_pnls: Vec<f64>,
}

/// Build tail-scenario stress attribution from position-level historical P&Ls.
///
/// The `position_pnls` buffer is row-major with shape
/// `n_scenarios × position_ids.len()`: for scenario `s` and position `i`, the
/// P&L is stored at `position_pnls[s * n_positions + i]`. Tail scenarios are
/// selected using the same boundary convention as
/// [`HistoricalPositionDecomposer::decompose_from_pnls`]: sort portfolio P&Ls
/// ascending, take the shared snapped-ceil tail count (see
/// `super::tail_scenario_count`) of scenarios, and set
/// `var_threshold` to the signed P&L of the least-bad tail scenario
/// (losses-negative convention).
///
/// # Errors
///
/// Returns a validation error when dimensions do not match, confidence is not
/// in `(0.5, 1)`, the requested tail has fewer than one scenario, or any P&L is
/// non-finite.
///
/// # Arguments
///
/// * `position_ids` - Position identifiers in the same column order as each
///   scenario row in `position_pnls`.
/// * `position_pnls` - Scenario-major flat P&L buffer with shape
///   `n_scenarios * position_ids.len()`; entries are reporting-currency P&L
///   amounts.
/// * `n_scenarios` - Number of scenario rows encoded in `position_pnls`.
/// * `confidence` - Tail confidence level strictly between 0.5 and 1.0, such
///   as `0.99` for a 99% stress-tail attribution.
pub fn build_stress_attribution(
    position_ids: &[String],
    position_pnls: &[f64],
    n_scenarios: usize,
    confidence: f64,
) -> finstack_quant_core::Result<StressAttribution> {
    let n_positions = position_ids.len();

    if n_positions == 0 || n_scenarios == 0 {
        if position_pnls.is_empty() {
            return Ok(StressAttribution {
                var_threshold: 0.0,
                n_tail_scenarios: 0,
                position_ids: Vec::new(),
                position_contributions: Vec::new(),
                tail_scenarios: Vec::new(),
            });
        }
        return Err(finstack_quant_core::Error::Validation(format!(
            "position_pnls length ({}) must be n_scenarios ({n_scenarios}) * n_positions ({n_positions})",
            position_pnls.len()
        )));
    }

    let expected_len = n_scenarios * n_positions;
    if position_pnls.len() != expected_len {
        return Err(finstack_quant_core::Error::Validation(format!(
            "position_pnls length ({}) must be n_scenarios ({n_scenarios}) * n_positions ({n_positions}) = {expected_len}",
            position_pnls.len()
        )));
    }

    if !confidence.is_finite() || confidence <= 0.5 || confidence >= 1.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "confidence must be finite and in (0.5, 1), got {confidence}"
        )));
    }

    if let Some(bad_idx) = position_pnls.iter().position(|p| !p.is_finite()) {
        let scenario = bad_idx / n_positions;
        let position = bad_idx % n_positions;
        return Err(finstack_quant_core::Error::Validation(format!(
            "position_pnls contains non-finite value at scenario {scenario}, position {position} (value = {})",
            position_pnls[bad_idx]
        )));
    }

    // Reject configurations whose exact tail is below one scenario before
    // taking the shared snapped-ceil count (which would round 0.5 up to 1).
    if ((1.0 - confidence) * (n_scenarios as f64)) < 1.0 - super::TAIL_COUNT_SNAP_TOLERANCE {
        return Err(finstack_quant_core::Error::Validation(format!(
            "confidence {confidence} with {n_scenarios} scenarios yields zero tail scenarios; lower confidence or provide more scenarios"
        )));
    }
    let n_tail = super::tail_scenario_count(confidence, n_scenarios);

    let mut portfolio_pnls: Vec<(usize, f64)> = (0..n_scenarios)
        .map(|scenario| {
            let row_start = scenario * n_positions;
            let pnl = position_pnls[row_start..row_start + n_positions]
                .iter()
                .sum();
            (scenario, pnl)
        })
        .collect();
    portfolio_pnls.sort_by(|a, b| a.1.total_cmp(&b.1));

    // Loss convention (workspace-wide): the threshold follows the P&L sign,
    // so tail losses are negative. Clamp to zero only if the boundary
    // scenario is actually a gain.
    let var_idx = (n_tail - 1).min(n_scenarios - 1);
    let var_threshold = portfolio_pnls[var_idx].1.min(0.0);

    let tail = &portfolio_pnls[..n_tail];
    let avg_tail_loss = -tail.iter().map(|(_, pnl)| *pnl).sum::<f64>() / n_tail as f64;

    let mut pnl_sums = vec![0.0; n_positions];
    let mut worst_pnls = vec![f64::INFINITY; n_positions];
    let mut tail_scenarios = Vec::with_capacity(n_tail);

    for &(scenario_index, portfolio_pnl) in tail {
        let row_start = scenario_index * n_positions;
        let row = &position_pnls[row_start..row_start + n_positions];
        for (idx, pnl) in row.iter().copied().enumerate() {
            pnl_sums[idx] += pnl;
            worst_pnls[idx] = worst_pnls[idx].min(pnl);
        }
        // Store only the `f64` row; the position id for column `i` is
        // `StressAttribution::position_ids[i]`, carried once on the parent
        // result instead of cloned into every tail scenario.
        tail_scenarios.push(TailScenarioBreakdown {
            scenario_index,
            portfolio_pnl,
            position_pnls: row.to_vec(),
        });
    }

    let mut position_contributions: Vec<StressPositionEntry> = position_ids
        .iter()
        .enumerate()
        .map(|(idx, position_id)| {
            let avg_tail_pnl = pnl_sums[idx] / n_tail as f64;
            let pct_of_tail_loss = if avg_tail_loss.abs() > f64::EPSILON {
                -avg_tail_pnl / avg_tail_loss
            } else {
                0.0
            };
            StressPositionEntry {
                position_id: position_id.clone(),
                avg_tail_pnl,
                pct_of_tail_loss,
                worst_scenario_pnl: worst_pnls[idx],
            }
        })
        .collect();

    position_contributions.sort_by(|a, b| {
        b.avg_tail_pnl
            .abs()
            .total_cmp(&a.avg_tail_pnl.abs())
            .then_with(|| a.position_id.as_str().cmp(b.position_id.as_str()))
    });

    Ok(StressAttribution {
        var_threshold,
        n_tail_scenarios: n_tail,
        position_ids: position_ids.to_vec(),
        position_contributions,
        tail_scenarios,
    })
}

// Shared math helpers

const VARIANCE_TOLERANCE: f64 = 1e-12;

use super::math::{normal_pdf, normal_quantile};

// Validation helpers

fn validate_decomposition_inputs(
    weights: &[f64],
    covariance: &[f64],
    position_ids: &[String],
    config: &DecompositionConfig,
) -> finstack_quant_core::Result<()> {
    let n = weights.len();

    if n != position_ids.len() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "weights length ({n}) must match position_ids length ({})",
            position_ids.len()
        )));
    }

    if covariance.len() != n * n {
        return Err(finstack_quant_core::Error::Validation(format!(
            "covariance length ({}) must be {}x{} = {}",
            covariance.len(),
            n,
            n,
            n * n
        )));
    }

    if !config.confidence.is_finite() || config.confidence <= 0.5 || config.confidence >= 1.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "confidence must be finite and in (0.5, 1), got {}",
            config.confidence
        )));
    }

    if weights.iter().any(|v| !v.is_finite()) {
        return Err(finstack_quant_core::Error::Validation(
            "weight entries must be finite".to_string(),
        ));
    }

    // Finiteness, symmetry and positive semi-definiteness via the
    // rank-tolerant Cholesky shared with the factor-level engine
    // (`ParametricDecomposer`). A strict positive-definite factorization
    // would reject rank-deficient PSD covariance — perfectly collinear
    // positions, factor structure `B Σ_f Bᵀ + D` with singular `Σ_f`, or
    // sample covariance with fewer observations than positions — all of
    // which are valid risk inputs: Euler allocation only requires
    // `σ_p = √(wᵀΣw) ≥ 0`.
    if n > 0 {
        super::math::cholesky(covariance, n)?;
    }

    Ok(())
}

// Incremental VaR

/// Compute incremental VaR for all positions in O(n) total.
///
/// Textbook definition: incremental VaR for position `k` is the change in
/// portfolio VaR caused by removing position `k`, with the remaining
/// positions held at their existing weights (no renormalization):
///
/// ```text
///   variance_excl_k = w' Σ w  -  2 w_k (Σ w)_k  +  w_k²  Σ_{kk}
///   var_excl_k      = z_α · sqrt(max(variance_excl_k, 0))
///   incremental_k   = portfolio_var - var_excl_k
/// ```
///
/// This matches Jorion (2007) §7.2.3 and the definition used in standard
/// risk-system reference implementations. It differs from the older
/// implementation in this file, which renormalized the remaining weights
/// by `S - w_k` (where `S = Σ_i w_i`). The renormalized form silently
/// magnifies `var_excl_k` when `S - w_k` is small and produces
/// counter-intuitive negative incrementals for non-diversifying positions;
/// it is not a textbook quantity.
///
/// The `sigma_w` argument must equal `Σ w`; this keeps the routine
/// allocation- and matrix-free.
fn compute_incremental_var(
    weights: &[f64],
    sigma_w: &[f64],
    covariance: &[f64],
    portfolio_variance: f64,
    portfolio_var: f64,
    confidence: f64,
    n: usize,
) -> Vec<f64> {
    let z_alpha = normal_quantile(confidence);

    (0..n)
        .map(|k| {
            let w_k = weights[k];
            let sw_k = sigma_w[k];
            let cov_kk = covariance[k * n + k];

            let variance_excl =
                (portfolio_variance - 2.0 * w_k * sw_k + w_k * w_k * cov_kk).max(0.0);

            // Loss-signed: VaR excluding k is also a negative number, so the
            // incremental (VaR_with − VaR_without) is negative for a
            // risk-adding position and positive for a diversifier.
            let var_excl = -(z_alpha * variance_excl.sqrt());
            portfolio_var - var_excl
        })
        .collect()
}

// Parametric engine

/// Parametric (covariance-based) position-level VaR decomposer.
///
/// Uses the multivariate normal assumption to decompose VaR and ES
/// analytically via Euler allocation. Fast and exact under normality.
///
/// # Mathematical Background
///
/// Under the normal model, portfolio return `r_p = w'r` has:
/// ```text
/// sigma_p = sqrt(w' * Sigma * w)
/// VaR_p   = -z_alpha * sigma_p  (zero-mean assumption; losses negative)
/// ```
///
/// The Euler decomposition exploits the positive homogeneity of VaR:
/// ```text
/// VaR_p = sum_i (w_i * dVaR/dw_i) = sum_i CVaR_i
/// ```
///
/// # References
///
/// - Litterman (1996): Hot Spots and Hedges. `docs/REFERENCES.md#litterman-1996-hotspots`
/// - Tasche (2008): Capital allocation with Euler's method. `docs/REFERENCES.md#tasche-2008-capital-allocation`
#[derive(Debug, Clone, Copy, Default)]
pub struct ParametricPositionDecomposer;

impl ParametricPositionDecomposer {
    /// Decompose portfolio VaR and ES into per-position contributions using Euler allocation.
    ///
    /// # Arguments
    ///
    /// * `weights` - Position weights as fraction of portfolio value (length `n_positions`).
    /// * `covariance` - Position-return covariance matrix (n x n, row-major, symmetric PSD).
    /// * `position_ids` - Position identifiers, aligned with `weights`.
    /// * `config` - Decomposition parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if dimensions are inconsistent, the covariance matrix is invalid, or
    /// the confidence level is out of bounds.
    pub fn decompose_positions(
        &self,
        weights: &[f64],
        covariance: &[f64],
        position_ids: &[String],
        config: &DecompositionConfig,
    ) -> finstack_quant_core::Result<PositionRiskDecomposition> {
        validate_decomposition_inputs(weights, covariance, position_ids, config)?;

        let n = weights.len();

        if n == 0 {
            return Ok(PositionRiskDecomposition {
                portfolio_var: 0.0,
                portfolio_es: 0.0,
                confidence: config.confidence,
                method: DecompositionMethod::Parametric,
                var_contributions: Vec::new(),
                es_contributions: Vec::new(),
                n_positions: 0,
                euler_residual: Some(0.0),
            });
        }

        let z_alpha = normal_quantile(config.confidence);
        let phi_z = normal_pdf(z_alpha);
        let es_multiplier = phi_z / (1.0 - config.confidence);

        // Sigma * w (matrix-vector product).
        let mut sigma_w = vec![0.0; n];
        for i in 0..n {
            let mut dot = 0.0;
            for j in 0..n {
                dot += covariance[i * n + j] * weights[j];
            }
            sigma_w[i] = dot;
        }

        // Portfolio variance = w' * Sigma * w. A materially negative value
        // means the covariance matrix is not PSD at the supplied weights —
        // reject it like the factor-level `ParametricDecomposer` does
        // (`validated_variance`) instead of clamping to a silent VaR of -0.
        // Only the numerical rounding band [-tolerance, 0) is clamped.
        let mut raw_variance = 0.0;
        for i in 0..n {
            raw_variance += weights[i] * sigma_w[i];
        }
        if raw_variance < -VARIANCE_TOLERANCE {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Portfolio variance must be non-negative, got {raw_variance}; covariance \
                 matrix is not positive semi-definite at the supplied weights"
            )));
        }
        let variance = raw_variance.max(0.0);
        let sigma_p = variance.sqrt();

        // Loss convention (workspace-wide): VaR and ES follow the P&L sign,
        // so losses are reported as negative numbers — matching the
        // factor-level engines and `analytics::value_at_risk`.
        let portfolio_var = -(sigma_p * z_alpha);
        let portfolio_es = -(sigma_p * es_multiplier);

        // Guard against zero-risk portfolio to avoid division by zero.
        let inv_sigma = if sigma_p > VARIANCE_TOLERANCE.sqrt() {
            1.0 / sigma_p
        } else {
            warn!(
                sigma_p,
                "parametric decomposer: portfolio sigma below sqrt(tolerance); marginal and \
                 component contributions will be zero. Portfolio may be degenerate or all \
                 weights near zero."
            );
            0.0
        };

        let mut var_contributions = Vec::with_capacity(n);
        let mut es_contributions = Vec::with_capacity(n);
        // Accumulated inline (ascending position order) so the Euler residual
        // below does not need a second pass over `var_contributions`.
        let mut sum_component_var = 0.0;

        for i in 0..n {
            // Component variance = w_i * (Sigma * w)_i.
            let cv_i = weights[i] * sigma_w[i];

            // Component VaR = -CV_i / sigma_p * z_alpha (loss-signed).
            let component_var = -(cv_i * inv_sigma * z_alpha);
            sum_component_var += component_var;

            // Marginal VaR = -(Sigma * w)_i / sigma_p * z_alpha (loss-signed).
            let marginal_var = -(sigma_w[i] * inv_sigma * z_alpha);

            // Relative VaR = CVaR_i / VaR_p.
            let relative_var = if portfolio_var.abs() > VARIANCE_TOLERANCE {
                component_var / portfolio_var
            } else {
                0.0
            };

            // Component ES = -CV_i / sigma_p * phi(z_alpha) / (1 - alpha) (loss-signed).
            let component_es = -(cv_i * inv_sigma * es_multiplier);

            // Marginal ES = -(Sigma * w)_i / sigma_p * phi(z) / (1 - alpha) (loss-signed).
            let marginal_es = -(sigma_w[i] * inv_sigma * es_multiplier);

            // Relative ES = CES_i / ES_p.
            let relative_es = if portfolio_es.abs() > VARIANCE_TOLERANCE {
                component_es / portfolio_es
            } else {
                0.0
            };

            var_contributions.push(PositionVarContribution {
                position_id: position_ids[i].clone(),
                component_var,
                relative_var,
                marginal_var: Some(marginal_var),
                incremental_var: None,
            });

            es_contributions.push(PositionEsContribution {
                position_id: position_ids[i].clone(),
                component_es,
                relative_es,
                marginal_es: Some(marginal_es),
            });
        }

        // Incremental VaR (expensive leave-one-out).
        if config.compute_incremental && n > 1 {
            let incremental = compute_incremental_var(
                weights,
                &sigma_w,
                covariance,
                variance,
                portfolio_var,
                config.confidence,
                n,
            );
            for (contribution, ivar) in var_contributions.iter_mut().zip(incremental) {
                contribution.incremental_var = Some(ivar);
            }
        } else if config.compute_incremental && n == 1 {
            // Single-position portfolio: incremental VaR equals portfolio VaR.
            var_contributions[0].incremental_var = Some(portfolio_var);
        }

        // Euler residual (parametric only; meaningful as a numerical diagnostic).
        // `sum_component_var` was accumulated inline above in the same ascending
        // position order this fold would use, so the result is unchanged.
        let euler_residual = Some(portfolio_var - sum_component_var);

        Ok(PositionRiskDecomposition {
            portfolio_var,
            portfolio_es,
            confidence: config.confidence,
            method: DecompositionMethod::Parametric,
            var_contributions,
            es_contributions,
            n_positions: n,
            euler_residual,
        })
    }
}

// Historical simulation engine

/// Historical simulation position-level VaR decomposer.
///
/// Decomposes VaR and ES by attributing portfolio losses to individual
/// positions within tail scenarios. The Euler property holds approximately
/// (exact in the limit of infinite scenarios).
///
/// # Algorithm
///
/// 1. Compute portfolio P&L for each scenario: PnL_p(s) = sum_i PnL_i(s)
/// 2. Sort scenarios by portfolio P&L (ascending = worst first)
/// 3. Identify tail: scenarios where PnL_p <= -VaR_p
/// 4. Component ES: CES_i = mean(-PnL_i(s)) for s in tail
/// 5. Component VaR: CVaR_i = CES_i * (VaR_p / ES_p)  (Tasche scaling)
///
/// # References
///
/// - Hallerbach (2003): Decomposing portfolio Value-at-Risk. `docs/REFERENCES.md#hallerbach-2003-decomposing-var`
///
#[derive(Debug, Clone, Default)]
pub struct HistoricalPositionDecomposer;

impl HistoricalPositionDecomposer {
    /// Decompose using pre-computed per-position scenario P&Ls.
    ///
    /// # Arguments
    ///
    /// * `position_pnls` - Matrix of per-position P&Ls, shape (n_scenarios, n_positions),
    ///   stored row-major. `position_pnls[s * n_positions + i]` is position `i`'s
    ///   P&L under scenario `s`.
    /// * `position_ids` - Position identifiers, length `n_positions`.
    /// * `n_scenarios` - Number of historical scenarios.
    /// * `config` - Decomposition parameters (only `confidence` is used;
    ///   `method` is ignored since this is always historical).
    ///
    /// # Errors
    ///
    /// Returns an error if dimensions are inconsistent, the number of
    /// scenarios is too small, or the confidence level is out of bounds.
    pub fn decompose_from_pnls(
        &self,
        position_pnls: &[f64],
        position_ids: &[String],
        n_scenarios: usize,
        config: &DecompositionConfig,
    ) -> finstack_quant_core::Result<PositionRiskDecomposition> {
        let n = position_ids.len();

        if position_pnls.len() != n_scenarios * n {
            return Err(finstack_quant_core::Error::Validation(format!(
                "position_pnls length ({}) must equal n_scenarios ({}) * n_positions ({})",
                position_pnls.len(),
                n_scenarios,
                n
            )));
        }

        if !config.confidence.is_finite() || config.confidence <= 0.5 || config.confidence >= 1.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "confidence must be finite and in (0.5, 1), got {}",
                config.confidence
            )));
        }

        if n == 0 || n_scenarios == 0 {
            return Ok(PositionRiskDecomposition {
                portfolio_var: 0.0,
                portfolio_es: 0.0,
                confidence: config.confidence,
                method: DecompositionMethod::Historical,
                var_contributions: Vec::new(),
                es_contributions: Vec::new(),
                n_positions: n,
                euler_residual: None,
            });
        }

        // Number of tail scenarios: shared snapped-ceil convention (see
        // `super::tail_scenario_count`), matching `SimulationDecomposer`.
        // Require at least two tail observations like the simulation engine:
        // a one-scenario tail collapses VaR and ES onto a single extreme
        // observation and cannot support a VaR/ES split.
        let n_tail = super::tail_scenario_count(config.confidence, n_scenarios);
        if n_tail < 2 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "historical decomposition requires at least two tail scenarios for confidence {} \
                 (got {n_tail} from {n_scenarios} scenarios); increase n_scenarios or lower \
                 the confidence level",
                config.confidence
            )));
        }
        if n_tail < 30 {
            warn!(
                n_tail,
                n_scenarios,
                confidence = config.confidence,
                "Tail sample size is small; historical VaR/ES decomposition may lack statistical reliability"
            );
        }

        // Pre-flight: any non-finite P&L corrupts the sort below
        // (`partial_cmp(NaN, _) = None`) and silently degrades the tail
        // ordering. Surface this as an explicit error so an upstream
        // numerical fault (e.g. a near-singular covariance feeding a Cholesky)
        // is caught rather than masked.
        if let Some(bad_idx) = position_pnls.iter().position(|p| !p.is_finite()) {
            let scenario = bad_idx / n;
            let position = bad_idx % n;
            return Err(finstack_quant_core::Error::Validation(format!(
                "position_pnls contains non-finite value at scenario {scenario}, \
                 position {position} (value = {}); upstream P&L generator must \
                 produce finite values",
                position_pnls[bad_idx]
            )));
        }

        let mut portfolio_pnls: Vec<(usize, f64)> = (0..n_scenarios)
            .map(|s| {
                let row_start = s * n;
                let pnl: f64 = position_pnls[row_start..row_start + n].iter().sum();
                (s, pnl)
            })
            .collect();

        // Sort ascending by portfolio P&L (worst first).
        portfolio_pnls.sort_by(|a, b| a.1.total_cmp(&b.1));

        // Portfolio VaR: the signed P&L at the tail boundary scenario. The
        // tail spans sorted indices 0..n_tail (ascending P&L), so the VaR
        // threshold is the least-bad scenario of the tail, index n_tail-1.
        //
        // Loss convention (workspace-wide): VaR/ES follow the P&L sign, so
        // losses are negative. Clamp to zero only when the quantile P&L is
        // actually a gain (extremely low confidence levels), matching
        // `SimulationDecomposer::tail_risk_decomposition`.
        let var_idx = (n_tail - 1).min(n_scenarios - 1);
        let portfolio_var = portfolio_pnls[var_idx].1.min(0.0);

        // Portfolio ES: average signed P&L in the tail scenarios.
        let raw_portfolio_es: f64 = portfolio_pnls[..n_tail]
            .iter()
            .map(|(_, pnl)| pnl)
            .sum::<f64>()
            / n_tail as f64;
        let portfolio_es = raw_portfolio_es.min(0.0);

        // Per-position Component ES: average of position-level losses in tail.
        //
        // A previous Rayon shard at `n_tail * n >= 100_000` lost throughput
        // versus this serial fold on measured books (400–600 positions ×
        // 4,000 scenarios). Keep the order-deterministic serial accumulation
        // over the sorted tail of `portfolio_pnls`.
        let mut component_es_vec = vec![0.0; n];
        for &(s, _) in &portfolio_pnls[..n_tail] {
            let row_start = s * n;
            for i in 0..n {
                component_es_vec[i] += position_pnls[row_start + i];
            }
        }
        for ces in component_es_vec.iter_mut() {
            *ces /= n_tail as f64;
        }

        // Gain-clamp Euler consistency: when the tail mean is a gain the
        // total ES clamps to zero above, so the components must be zeroed in
        // the same branch or they no longer sum to the total (matching
        // `SimulationDecomposer::tail_risk_decomposition`).
        if raw_portfolio_es > 0.0 {
            component_es_vec.fill(0.0);
        }

        // Component VaR via Tasche scaling: CVaR_i = CES_i * (VaR / ES).
        // Degenerate ES (~0): no proration — component VaR is zeroed rather
        // than copied from the ES components, matching the simulation
        // engine's fallback of 0.
        let var_es_ratio = if portfolio_es.abs() > VARIANCE_TOLERANCE {
            portfolio_var / portfolio_es
        } else {
            0.0
        };
        let component_var_vec: Vec<f64> = component_es_vec
            .iter()
            .map(|ces| ces * var_es_ratio)
            .collect();

        // Marginal VaR/ES are not analytically available from raw scenario
        // P&Ls: they require either position weights (to differentiate)
        // or a finite-difference repricing engine. Report None rather than
        // a misleading proxy value.
        let mut var_contributions = Vec::with_capacity(n);
        let mut es_contributions = Vec::with_capacity(n);

        for i in 0..n {
            let relative_var = if portfolio_var.abs() > VARIANCE_TOLERANCE {
                component_var_vec[i] / portfolio_var
            } else {
                0.0
            };

            let relative_es = if portfolio_es.abs() > VARIANCE_TOLERANCE {
                component_es_vec[i] / portfolio_es
            } else {
                0.0
            };

            var_contributions.push(PositionVarContribution {
                position_id: position_ids[i].clone(),
                component_var: component_var_vec[i],
                relative_var,
                marginal_var: None,
                incremental_var: None,
            });

            es_contributions.push(PositionEsContribution {
                position_id: position_ids[i].clone(),
                component_es: component_es_vec[i],
                relative_es,
                marginal_es: None,
            });
        }

        // Euler residual is algebraically zero in historical mode because
        // CVaR_i = CES_i * (VaR/ES) and sum(CES_i) = ES by construction.
        // Reporting it as None avoids implying a diagnostic that does not
        // exist here.
        Ok(PositionRiskDecomposition {
            portfolio_var,
            portfolio_es,
            confidence: config.confidence,
            method: DecompositionMethod::Historical,
            var_contributions,
            es_contributions,
            n_positions: n,
            euler_residual: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = finstack_quant_core::Result<()>;

    // Parametric tests

    #[test]
    fn euler_exhaustion_two_position_portfolio() -> TestResult {
        // Two uncorrelated assets: sigma1 = 0.20, sigma2 = 0.30.
        // Weights: 0.6, 0.4.
        let weights = [0.6, 0.4];
        let covariance = [0.04, 0.0, 0.0, 0.09];
        let ids = [String::from("A"), String::from("B")];
        let config = DecompositionConfig::parametric_99();

        let decomposer = ParametricPositionDecomposer;
        let result = decomposer.decompose_positions(&weights, &covariance, &ids, &config)?;

        let sum_cvar: f64 = result
            .var_contributions
            .iter()
            .map(|c| c.component_var)
            .sum();
        assert!(
            (sum_cvar - result.portfolio_var).abs() < 1e-10,
            "Euler exhaustion failed: sum={sum_cvar}, total={}",
            result.portfolio_var
        );

        let sum_rel: f64 = result
            .var_contributions
            .iter()
            .map(|c| c.relative_var)
            .sum();
        assert!(
            (sum_rel - 1.0).abs() < 1e-10,
            "relative VaR sum failed: {sum_rel}"
        );

        let residual = result
            .euler_residual
            .expect("parametric decomposition must report euler_residual");
        assert!(residual.abs() < 1e-10, "euler_residual = {residual}");

        Ok(())
    }

    /// Workspace sign convention: VaR and ES follow the P&L sign, so losses
    /// are reported as **negative** numbers — matching the factor-level
    /// engines (`ParametricDecomposer`, `SimulationDecomposer`) and
    /// `analytics::value_at_risk`. Component/marginal contributions carry
    /// the same sign so Euler exhaustion holds with signed totals.
    #[test]
    fn parametric_var_and_es_report_losses_as_negative() -> TestResult {
        let weights = [0.6, 0.4];
        let covariance = [0.04, 0.0, 0.0, 0.09];
        let ids = [String::from("A"), String::from("B")];
        let config = DecompositionConfig::parametric_99();

        let result = ParametricPositionDecomposer.decompose_positions(
            &weights,
            &covariance,
            &ids,
            &config,
        )?;

        // sigma_p = sqrt(0.36*0.04 + 0.16*0.09) = sqrt(0.0288)
        let sigma_p = 0.0288_f64.sqrt();
        let z_99 = 2.326_347_874_040_840_8;
        assert!(
            (result.portfolio_var - (-sigma_p * z_99)).abs() < 1e-6,
            "VaR must be negative (losses-negative convention), got {}",
            result.portfolio_var
        );
        assert!(result.portfolio_var < 0.0);
        assert!(result.portfolio_es < result.portfolio_var, "ES beyond VaR");

        // Contributions carry the loss sign; Euler exhaustion on signed totals.
        let sum_cvar: f64 = result
            .var_contributions
            .iter()
            .map(|c| c.component_var)
            .sum();
        assert!((sum_cvar - result.portfolio_var).abs() < 1e-10);
        for c in &result.var_contributions {
            assert!(c.component_var < 0.0, "long risk contributes losses");
            // relative share stays a positive fraction (signs cancel).
            assert!(c.relative_var > 0.0);
        }
        Ok(())
    }

    /// Historical decomposition follows the same losses-negative convention:
    /// the VaR is the signed P&L at the tail quantile, ES the signed tail
    /// mean, matching `SimulationDecomposer::tail_risk_decomposition`.
    #[test]
    fn historical_var_and_es_report_losses_as_negative() -> TestResult {
        // 200 scenarios, single position, P&L = -50..149 (worst = -50).
        // (200 rather than 100 scenarios: the historical engine now requires
        // at least two tail observations, like the simulation engine.)
        let n_scenarios = 200;
        let pnls: Vec<f64> = (0..n_scenarios).map(|s| s as f64 - 50.0).collect();
        let ids = [String::from("A")];
        let config = DecompositionConfig {
            confidence: 0.99,
            method: DecompositionMethod::Historical,
            compute_incremental: false,
        };

        let result =
            HistoricalPositionDecomposer.decompose_from_pnls(&pnls, &ids, n_scenarios, &config)?;

        // 1% tail of 200 scenarios = 2 scenarios (-50, -49): VaR is the
        // boundary P&L, ES the tail mean.
        assert!(
            (result.portfolio_var - (-49.0)).abs() < 1e-12,
            "historical VaR must be the signed tail P&L, got {}",
            result.portfolio_var
        );
        assert!((result.portfolio_es - (-49.5)).abs() < 1e-12);
        let sum_ces: f64 = result.es_contributions.iter().map(|c| c.component_es).sum();
        assert!((sum_ces - result.portfolio_es).abs() < 1e-9);
        Ok(())
    }

    /// Rank-deficient PSD covariance (perfectly collinear positions) is a
    /// valid risk input — factor covariance `B Σ_f Bᵀ + D` can be exactly
    /// singular and sample covariance with T < N always is. The position
    /// decomposer must accept it like the factor-level engines do
    /// (see `parametric::tests::test_parametric_accepts_rank_deficient_psd_covariance`);
    /// Euler allocation only requires `σ_p = √(wᵀΣw) ≥ 0` (Meucci, *Risk
    /// and Asset Allocation* §4).
    #[test]
    fn accepts_rank_deficient_psd_covariance() -> TestResult {
        // Two perfectly correlated positions with identical variance:
        // covariance is PSD but rank 1. Dyadic-exact entries (0.25, whose
        // square root 0.5 is exact in binary) make the second Cholesky
        // pivot exactly zero, so a strict positive-definite factorization
        // deterministically rejects this matrix rather than escaping via
        // floating-point rounding noise.
        let weights = [0.5, 0.5];
        let covariance = [0.25, 0.25, 0.25, 0.25];
        let ids = [String::from("A"), String::from("A2")];
        let config = DecompositionConfig::parametric_99();

        let decomposer = ParametricPositionDecomposer;
        let result = decomposer.decompose_positions(&weights, &covariance, &ids, &config)?;

        // sigma_p = sqrt(0.25*0.25 + 2*0.25*0.25 + 0.25*0.25) = 0.5;
        // losses-negative convention: VaR = -sigma_p * z.
        let z_99 = 2.326_347_874_040_840_8;
        assert!((result.portfolio_var - (-0.5 * z_99)).abs() < 1e-6);

        // Euler exhaustion still holds on the singular direction.
        let sum_cvar: f64 = result
            .var_contributions
            .iter()
            .map(|c| c.component_var)
            .sum();
        assert!((sum_cvar - result.portfolio_var).abs() < 1e-10);

        Ok(())
    }

    #[test]
    fn equal_weight_equal_vol_has_equal_component_var() -> TestResult {
        // Three assets, all identical vol, zero correlation.
        let vol = 0.15;
        let var = vol * vol;
        let n = 3;
        let w = 1.0 / n as f64;
        let weights = vec![w; n];
        let mut covariance = vec![0.0; n * n];
        for i in 0..n {
            covariance[i * n + i] = var;
        }
        let ids: Vec<String> = (0..n).map(|i| format!("P{i}")).collect();
        let config = DecompositionConfig::parametric_95();

        let decomposer = ParametricPositionDecomposer;
        let result = decomposer.decompose_positions(&weights, &covariance, &ids, &config)?;

        let first_cvar = result.var_contributions[0].component_var;
        for c in &result.var_contributions {
            assert!(
                (c.component_var - first_cvar).abs() < 1e-12,
                "unequal component VaR: {} vs {first_cvar}",
                c.component_var
            );
        }

        for c in &result.var_contributions {
            assert!(
                (c.relative_var - w).abs() < 1e-12,
                "relative VaR {} != expected {w}",
                c.relative_var
            );
        }

        Ok(())
    }

    #[test]
    fn single_position_portfolio() -> TestResult {
        let weights = [1.0];
        let covariance = [0.04]; // sigma = 0.20
        let ids = [String::from("SOLO")];
        let config = DecompositionConfig::parametric_95().with_incremental();

        let decomposer = ParametricPositionDecomposer;
        let result = decomposer.decompose_positions(&weights, &covariance, &ids, &config)?;

        assert!((result.var_contributions[0].component_var - result.portfolio_var).abs() < 1e-12);

        // Single-position, weight = 1: marginal VaR equals portfolio VaR.
        let mvar = result.var_contributions[0]
            .marginal_var
            .expect("parametric: marginal_var must be Some");
        assert!((mvar - result.portfolio_var).abs() < 1e-12);

        let ivar = result.var_contributions[0]
            .incremental_var
            .unwrap_or(f64::NAN);
        assert!(
            (ivar - result.portfolio_var).abs() < 1e-12,
            "incremental VaR {ivar} != portfolio VaR {}",
            result.portfolio_var
        );

        assert!((result.var_contributions[0].relative_var - 1.0).abs() < 1e-12);

        Ok(())
    }

    #[test]
    fn zero_weight_position_has_zero_contributions() -> TestResult {
        let weights = [1.0, 0.0];
        let covariance = [0.04, 0.01, 0.01, 0.09];
        let ids = [String::from("A"), String::from("ZERO")];
        let config = DecompositionConfig::parametric_95();

        let decomposer = ParametricPositionDecomposer;
        let result = decomposer.decompose_positions(&weights, &covariance, &ids, &config)?;

        let zero_pos = &result.var_contributions[1];
        assert!(
            zero_pos.component_var.abs() < 1e-12,
            "zero-weight component VaR = {}",
            zero_pos.component_var
        );
        assert!(!zero_pos.component_var.is_nan());
        let mvar = zero_pos
            .marginal_var
            .expect("parametric: marginal_var must be Some");
        assert!(!mvar.is_nan());
        assert!(!zero_pos.relative_var.is_nan());

        Ok(())
    }

    #[test]
    fn es_ge_var_for_all_positions() -> TestResult {
        // Losses-negative convention: ES is beyond VaR in the loss tail,
        // i.e. ES <= VaR as signed numbers.
        let weights = [0.4, 0.3, 0.3];
        let covariance = [0.04, 0.01, 0.005, 0.01, 0.09, 0.02, 0.005, 0.02, 0.0625];
        let ids = [String::from("A"), String::from("B"), String::from("C")];
        let config = DecompositionConfig::parametric_99();

        let decomposer = ParametricPositionDecomposer;
        let result = decomposer.decompose_positions(&weights, &covariance, &ids, &config)?;

        assert!(
            result.portfolio_es <= result.portfolio_var,
            "portfolio ES ({}) < VaR ({})",
            result.portfolio_es,
            result.portfolio_var
        );

        for (vc, ec) in result
            .var_contributions
            .iter()
            .zip(result.es_contributions.iter())
        {
            // For a risk-adding (loss-signed, negative) component VaR, the
            // ES component lies beyond it in the loss tail.
            if vc.component_var < 0.0 {
                assert!(
                    ec.component_es <= vc.component_var + 1e-12,
                    "position {} ES ({}) < VaR ({})",
                    vc.position_id,
                    ec.component_es,
                    vc.component_var
                );
            }
        }

        Ok(())
    }

    #[test]
    fn negative_correlation_shows_diversification() -> TestResult {
        // Two positions with high negative correlation.
        let weights = [0.5, 0.5];
        // sigma1 = 0.2, sigma2 = 0.2, rho = -0.8
        // cov(1,2) = rho * sigma1 * sigma2 = -0.8 * 0.04 = -0.032
        let covariance = [0.04, -0.032, -0.032, 0.04];
        let ids = [String::from("A"), String::from("B")];
        let config = DecompositionConfig::parametric_95();

        let decomposer = ParametricPositionDecomposer;
        let result = decomposer.decompose_positions(&weights, &covariance, &ids, &config)?;

        // Diversification: the portfolio loses less than the sum of
        // standalone losses, so the signed portfolio VaR sits above the
        // (negative) sum of standalone VaRs.
        let z = normal_quantile(0.95);
        let standalone_var_a = -(0.5 * 0.2 * z);
        let standalone_var_b = -(0.5 * 0.2 * z);
        let sum_standalone = standalone_var_a + standalone_var_b;

        assert!(
            result.portfolio_var > sum_standalone,
            "portfolio VaR ({}) should show diversification vs standalone sum ({sum_standalone})",
            result.portfolio_var
        );

        // Both component VaRs stay loss-signed (negative) even with
        // negative correlation: each long position still adds risk here.
        for c in &result.var_contributions {
            assert!(
                c.component_var < 0.0,
                "component VaR for {} should be negative: {}",
                c.position_id,
                c.component_var
            );
        }

        // Euler still holds.
        let sum_cvar: f64 = result
            .var_contributions
            .iter()
            .map(|c| c.component_var)
            .sum();
        assert!((sum_cvar - result.portfolio_var).abs() < 1e-10);

        Ok(())
    }

    #[test]
    fn euler_exhaustion_five_positions() -> TestResult {
        // 5-position portfolio with a realistic covariance structure.
        let weights = [0.15, 0.25, 0.20, 0.25, 0.15];
        let n = 5;
        // Lower triangular L (hand-crafted to ensure PSD).
        #[rustfmt::skip]
        let l = [
            0.20, 0.00, 0.00, 0.00, 0.00,
            0.05, 0.18, 0.00, 0.00, 0.00,
            0.03, 0.04, 0.22, 0.00, 0.00,
            0.02, 0.06, 0.03, 0.15, 0.00,
            0.01, 0.02, 0.05, 0.04, 0.19,
        ];
        // Sigma = L * L'.
        let mut covariance = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                let mut sum = 0.0;
                for k in 0..n {
                    sum += l[i * n + k] * l[j * n + k];
                }
                covariance[i * n + j] = sum;
            }
        }

        let ids: Vec<String> = (0..n).map(|i| format!("P{i}")).collect();
        let config = DecompositionConfig::parametric_99();

        let decomposer = ParametricPositionDecomposer;
        let result = decomposer.decompose_positions(&weights, &covariance, &ids, &config)?;

        let sum_cvar: f64 = result
            .var_contributions
            .iter()
            .map(|c| c.component_var)
            .sum();
        assert!(
            (sum_cvar - result.portfolio_var).abs() < 1e-10,
            "5-pos Euler exhaustion: sum={sum_cvar}, total={}",
            result.portfolio_var
        );

        let sum_ces: f64 = result.es_contributions.iter().map(|c| c.component_es).sum();
        assert!(
            (sum_ces - result.portfolio_es).abs() < 1e-10,
            "5-pos ES Euler exhaustion: sum={sum_ces}, total={}",
            result.portfolio_es
        );

        Ok(())
    }

    #[test]
    fn empty_portfolio_returns_zero() -> TestResult {
        let decomposer = ParametricPositionDecomposer;
        let result =
            decomposer.decompose_positions(&[], &[], &[], &DecompositionConfig::parametric_95())?;

        assert!(result.portfolio_var.abs() < 1e-12);
        assert!(result.portfolio_es.abs() < 1e-12);
        assert_eq!(result.n_positions, 0);
        assert!(result.var_contributions.is_empty());
        assert!(result.es_contributions.is_empty());

        Ok(())
    }

    #[test]
    fn rejects_mismatched_dimensions() {
        let decomposer = ParametricPositionDecomposer;

        // Weights longer than position_ids.
        let result = decomposer.decompose_positions(
            &[0.5, 0.5],
            &[0.04, 0.0, 0.0, 0.04],
            &[String::from("A")],
            &DecompositionConfig::parametric_95(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn rejects_invalid_confidence() {
        let decomposer = ParametricPositionDecomposer;
        let mut config = DecompositionConfig::parametric_95();
        config.confidence = 1.5;

        let result = decomposer.decompose_positions(&[1.0], &[0.04], &[String::from("A")], &config);
        assert!(result.is_err());
    }

    #[test]
    fn minor22_23_rejects_sub_median_and_nan_confidence() {
        let decomposer = ParametricPositionDecomposer;
        for confidence in [0.5, f64::NAN] {
            let mut config = DecompositionConfig::parametric_95();
            config.confidence = confidence;
            let result =
                decomposer.decompose_positions(&[1.0], &[0.04], &[String::from("A")], &config);
            assert!(
                result.is_err(),
                "minor 22/23: confidence {confidence:?} must fail"
            );
        }

        let historical = HistoricalPositionDecomposer;
        for confidence in [0.5, f64::NAN] {
            let mut config = DecompositionConfig::historical(0.95);
            config.confidence = confidence;
            let result =
                historical.decompose_from_pnls(&[-1.0, -2.0], &[String::from("A")], 2, &config);
            assert!(
                result.is_err(),
                "minor 22/23: historical confidence {confidence:?} must fail"
            );
        }
    }

    #[test]
    fn incremental_var_three_positions() -> TestResult {
        let weights = [0.4, 0.35, 0.25];
        let covariance = [0.04, 0.01, 0.005, 0.01, 0.09, 0.02, 0.005, 0.02, 0.0625];
        let ids = [String::from("A"), String::from("B"), String::from("C")];
        let config = DecompositionConfig::parametric_99().with_incremental();

        let decomposer = ParametricPositionDecomposer;
        let result = decomposer.decompose_positions(&weights, &covariance, &ids, &config)?;

        for c in &result.var_contributions {
            assert!(
                c.incremental_var.is_some(),
                "incremental VaR missing for {}",
                c.position_id
            );
        }

        for c in &result.var_contributions {
            let ivar = c.incremental_var.unwrap_or(f64::NAN);
            assert!(
                ivar.is_finite(),
                "incremental VaR for {} should be finite: {ivar}",
                c.position_id
            );
        }

        // Position B (highest standalone vol = 0.30) should have the most
        // negative incremental VaR since removing it reduces risk most
        // (losses-negative convention).
        let ivar_b = result.var_contributions[1].incremental_var.unwrap_or(0.0);
        let ivar_c = result.var_contributions[2].incremental_var.unwrap_or(0.0);
        assert!(
            ivar_b < ivar_c,
            "position B (higher vol) should have a more negative incremental VaR than C: B={ivar_b}, C={ivar_c}"
        );

        Ok(())
    }

    // Historical decomposition tests

    #[test]
    fn historical_decomposition_basic() -> TestResult {
        // 100 scenarios, 2 positions.
        // Position A: steady small losses around -0.01.
        // Position B: occasional large losses.
        let n = 2;
        let n_scenarios = 100;
        let mut pnls = Vec::with_capacity(n_scenarios * n);

        for s in 0..n_scenarios {
            let a_pnl = -0.01 + 0.001 * (s as f64 / 10.0).sin();
            let b_pnl = if s < 5 {
                -0.10 // Tail scenario for B.
            } else {
                0.005 + 0.002 * (s as f64 / 5.0).cos()
            };
            pnls.push(a_pnl);
            pnls.push(b_pnl);
        }

        let ids = [String::from("A"), String::from("B")];
        let config = DecompositionConfig::historical(0.95);

        let decomposer = HistoricalPositionDecomposer;
        let result = decomposer.decompose_from_pnls(&pnls, &ids, n_scenarios, &config)?;

        assert!(
            result.portfolio_var < 0.0,
            "portfolio VaR should be negative (losses-negative convention)"
        );
        assert!(
            result.portfolio_es <= result.portfolio_var,
            "ES should lie at or beyond VaR in the loss tail"
        );
        assert_eq!(result.n_positions, 2);
        assert_eq!(result.method, DecompositionMethod::Historical);

        Ok(())
    }

    #[test]
    fn historical_rejects_dimension_mismatch() {
        let decomposer = HistoricalPositionDecomposer;
        let result = decomposer.decompose_from_pnls(
            &[1.0, 2.0, 3.0], // 3 values, but 2 scenarios x 2 positions = 4.
            &[String::from("A"), String::from("B")],
            2,
            &DecompositionConfig::historical(0.95),
        );
        assert!(result.is_err());
    }

    #[test]
    fn historical_empty_returns_zero() -> TestResult {
        let decomposer = HistoricalPositionDecomposer;
        let result =
            decomposer.decompose_from_pnls(&[], &[], 0, &DecompositionConfig::historical(0.95))?;

        assert!(result.portfolio_var.abs() < 1e-12);
        assert_eq!(result.n_positions, 0);
        Ok(())
    }

    #[test]
    fn stress_attribution_uses_historical_tail_boundary() -> TestResult {
        let ids = [String::from("A"), String::from("B")];
        let n_scenarios = 20;
        let mut pnls = Vec::with_capacity(n_scenarios * ids.len());
        for scenario in 0..n_scenarios {
            match scenario {
                0 => {
                    pnls.push(-8.0);
                    pnls.push(-2.0);
                }
                1 => {
                    pnls.push(-3.0);
                    pnls.push(-1.0);
                }
                _ => {
                    pnls.push(1.0);
                    pnls.push(0.5);
                }
            }
        }

        let attr = build_stress_attribution(&ids, &pnls, n_scenarios, 0.95)?;

        assert_eq!(attr.n_tail_scenarios, 1);
        assert!((attr.var_threshold - (-10.0)).abs() < 1e-12);
        assert_eq!(attr.tail_scenarios[0].scenario_index, 0);
        assert_eq!(attr.tail_scenarios[0].portfolio_pnl, -10.0);
        // Per-position P&L is index-aligned to the shared `position_ids` list
        // carried once on the parent (no per-scenario id duplication).
        assert_eq!(
            attr.position_ids,
            vec![String::from("A"), String::from("B")]
        );
        assert_eq!(attr.tail_scenarios[0].position_pnls, vec![-8.0, -2.0]);
        assert_eq!(
            attr.position_contributions[0].position_id,
            String::from("A")
        );
        assert!((attr.position_contributions[0].avg_tail_pnl + 8.0).abs() < 1e-12);
        assert!((attr.position_contributions[0].pct_of_tail_loss - 0.8).abs() < 1e-12);
        assert!((attr.position_contributions[1].pct_of_tail_loss - 0.2).abs() < 1e-12);

        Ok(())
    }

    #[test]
    fn stress_attribution_averages_multiple_tail_scenarios() -> TestResult {
        let ids = [String::from("A"), String::from("B")];
        let n_scenarios = 40;
        let mut pnls = Vec::with_capacity(n_scenarios * ids.len());
        for scenario in 0..n_scenarios {
            match scenario {
                0 => {
                    pnls.push(-8.0);
                    pnls.push(-2.0);
                }
                1 => {
                    pnls.push(-2.0);
                    pnls.push(-4.0);
                }
                _ => {
                    pnls.push(0.5);
                    pnls.push(0.5);
                }
            }
        }

        let attr = build_stress_attribution(&ids, &pnls, n_scenarios, 0.95)?;

        assert_eq!(attr.n_tail_scenarios, 2);
        assert!((attr.var_threshold - (-6.0)).abs() < 1e-12);
        assert_eq!(attr.tail_scenarios[0].scenario_index, 0);
        assert_eq!(attr.tail_scenarios[1].scenario_index, 1);

        let contrib_a = attr
            .position_contributions
            .iter()
            .find(|entry| entry.position_id == "A")
            .expect("A contribution should exist");
        let contrib_b = attr
            .position_contributions
            .iter()
            .find(|entry| entry.position_id == "B")
            .expect("B contribution should exist");

        assert!((contrib_a.avg_tail_pnl + 5.0).abs() < 1e-12);
        assert!((contrib_b.avg_tail_pnl + 3.0).abs() < 1e-12);
        assert!((contrib_a.pct_of_tail_loss - 0.625).abs() < 1e-12);
        assert!((contrib_b.pct_of_tail_loss - 0.375).abs() < 1e-12);

        Ok(())
    }

    #[test]
    fn stress_attribution_rejects_underspecified_tail() {
        let ids = [String::from("A")];
        let pnls = vec![0.0; 50];
        let result = build_stress_attribution(&ids, &pnls, 50, 0.99);
        assert!(result.is_err());
    }

    // C1 regression: VaR quantile index is the boundary of the tail, not
    // one-past-the-end. With 100 equally-spaced sorted P&Ls and 95%
    // confidence, the tail spans indices 0..5; VaR = -pnl[4] (index n_tail-1),
    // not -pnl[5].
    #[test]
    fn historical_var_uses_boundary_tail_index() -> TestResult {
        // 100 scenarios, single position with deterministic P&Ls:
        // pnl[s] = s as f64 / 100.0 - 0.5, so sorted ascending is
        // [-0.50, -0.49, ..., -0.46, -0.45, ...].
        let n_scenarios = 100;
        let n = 1;
        let mut pnls = Vec::with_capacity(n_scenarios * n);
        for s in 0..n_scenarios {
            pnls.push(s as f64 / 100.0 - 0.5);
        }

        let ids = [String::from("X")];
        let config = DecompositionConfig::historical(0.95);

        let decomposer = HistoricalPositionDecomposer;
        let result = decomposer.decompose_from_pnls(&pnls, &ids, n_scenarios, &config)?;

        // n_tail = 5, var_idx = 4, sorted pnl[4] = 4/100 - 0.5 = -0.46.
        // Losses-negative convention: portfolio VaR = -0.46.
        assert!(
            (result.portfolio_var - (-0.46)).abs() < 1e-12,
            "portfolio_var = {}, expected -0.46 (boundary index 4)",
            result.portfolio_var
        );

        Ok(())
    }

    // C2 regression: reject configurations where the tail is too small
    // to resolve (e.g. 99% confidence with 50 scenarios: 0.01 * 50 = 0.5 < 1).
    #[test]
    fn historical_rejects_underspecified_tail() {
        let n_scenarios = 50;
        let n = 1;
        let pnls = vec![0.0; n_scenarios * n];
        let ids = [String::from("X")];
        let config = DecompositionConfig::historical(0.99);

        let decomposer = HistoricalPositionDecomposer;
        let result = decomposer.decompose_from_pnls(&pnls, &ids, n_scenarios, &config);
        assert!(
            result.is_err(),
            "expected rejection when (1 - conf) * n_scenarios < 1"
        );
    }

    // C3/C4 regression: historical mode must report None for marginal
    // VaR, marginal ES, and euler_residual (none are meaningful in that
    // mode without additional inputs).
    #[test]
    fn historical_reports_none_for_marginals_and_residual() -> TestResult {
        let n = 2;
        let n_scenarios = 200;
        let mut pnls = Vec::with_capacity(n_scenarios * n);
        for s in 0..n_scenarios {
            pnls.push(-0.01 + 0.001 * (s as f64 / 10.0).sin());
            pnls.push(if s < 10 { -0.10 } else { 0.005 });
        }
        let ids = [String::from("A"), String::from("B")];
        let config = DecompositionConfig::historical(0.95);

        let decomposer = HistoricalPositionDecomposer;
        let result = decomposer.decompose_from_pnls(&pnls, &ids, n_scenarios, &config)?;

        assert!(
            result.euler_residual.is_none(),
            "historical euler_residual must be None"
        );
        for c in &result.var_contributions {
            assert!(
                c.marginal_var.is_none(),
                "historical marginal_var must be None for position {}",
                c.position_id
            );
        }
        for c in &result.es_contributions {
            assert!(
                c.marginal_es.is_none(),
                "historical marginal_es must be None for position {}",
                c.position_id
            );
        }

        Ok(())
    }

    // W1 regression: incremental VaR uses the textbook (non-renormalized)
    // definition, so for a long-only portfolio with positive-variance
    // positions the incremental VaR for each position must be non-negative
    // (removing a risky position cannot increase portfolio VaR).
    #[test]
    fn incremental_var_non_positive_for_long_only_portfolio() -> TestResult {
        let weights = [0.4, 0.35, 0.25];
        let covariance = [0.04, 0.01, 0.005, 0.01, 0.09, 0.02, 0.005, 0.02, 0.0625];
        let ids = [String::from("A"), String::from("B"), String::from("C")];
        let config = DecompositionConfig::parametric_99().with_incremental();

        let decomposer = ParametricPositionDecomposer;
        let result = decomposer.decompose_positions(&weights, &covariance, &ids, &config)?;

        for c in &result.var_contributions {
            let ivar = c
                .incremental_var
                .expect("incremental VaR must be present when requested");
            assert!(
                ivar <= 1e-12,
                "long-only incremental VaR for {} must be non-positive \
                 (losses-negative convention), got {ivar}",
                c.position_id
            );
            // Textbook bound: |incremental_k| <= |portfolio_var|, i.e. the
            // signed incremental is bounded below by the signed portfolio
            // VaR (var_excl is at most zero when the remaining weights are
            // perfectly hedged, which isn't true here).
            assert!(
                ivar >= result.portfolio_var - 1e-12,
                "incremental VaR for {} exceeds portfolio VaR: ivar={ivar}, pvar={}",
                c.position_id,
                result.portfolio_var
            );
        }

        Ok(())
    }

    // Parametric mode must report Some for marginals and residual.
    #[test]
    fn parametric_reports_some_for_marginals_and_residual() -> TestResult {
        let weights = [0.6, 0.4];
        let covariance = [0.04, 0.0, 0.0, 0.09];
        let ids = [String::from("A"), String::from("B")];
        let config = DecompositionConfig::parametric_95();

        let decomposer = ParametricPositionDecomposer;
        let result = decomposer.decompose_positions(&weights, &covariance, &ids, &config)?;

        assert!(
            result.euler_residual.is_some(),
            "parametric euler_residual must be Some"
        );
        for c in &result.var_contributions {
            assert!(
                c.marginal_var.is_some(),
                "parametric marginal_var must be Some for {}",
                c.position_id
            );
        }
        for c in &result.es_contributions {
            assert!(
                c.marginal_es.is_some(),
                "parametric marginal_es must be Some for {}",
                c.position_id
            );
        }

        Ok(())
    }

    /// Component ES must match a hand-rolled serial accumulation over the
    /// sorted tail. The production path is serial: a previous Rayon shard
    /// lost throughput on measured books.
    #[test]
    fn historical_tail_component_es_matches_sorted_serial_fold() -> TestResult {
        let n_scenarios = 4_000_usize;
        let n = 64_usize;
        let confidence = 0.95_f64;

        // Build a deterministic synthetic P&L matrix so the test is
        // reproducible and covers a mix of profits and losses.
        let mut pnls = Vec::with_capacity(n_scenarios * n);
        for s in 0..n_scenarios {
            for i in 0..n {
                let v = ((s as f64 * 0.013) - (i as f64 * 0.007)).sin() * 1_000.0;
                pnls.push(v);
            }
        }
        let ids: Vec<String> = (0..n).map(|i| format!("P{i}")).collect();
        let mut config = DecompositionConfig::historical(confidence);
        config.confidence = confidence;

        let decomposer = HistoricalPositionDecomposer;
        let result = decomposer.decompose_from_pnls(&pnls, &ids, n_scenarios, &config)?;

        // Serial reference: replicate the inner accumulation directly so we
        // know exactly which order was used.
        let mut portfolio_pnls: Vec<(usize, f64)> = (0..n_scenarios)
            .map(|s| {
                let row_start = s * n;
                let pnl: f64 = pnls[row_start..row_start + n].iter().sum();
                (s, pnl)
            })
            .collect();
        portfolio_pnls.sort_by(|a, b| a.1.total_cmp(&b.1));
        let n_tail = ((1.0 - confidence) * n_scenarios as f64).floor() as usize;
        let mut serial_ces = vec![0.0_f64; n];
        for &(s, _) in &portfolio_pnls[..n_tail] {
            let row_start = s * n;
            for i in 0..n {
                serial_ces[i] += pnls[row_start + i];
            }
        }
        for v in serial_ces.iter_mut() {
            *v /= n_tail as f64;
        }

        for (i, contrib) in result.es_contributions.iter().enumerate() {
            let got = contrib.component_es;
            let ser = serial_ces[i];
            let scale = got.abs().max(ser.abs()).max(1.0);
            assert!(
                (got - ser).abs() <= 1e-9 * scale,
                "component ES diverged at position {i}: \
                 serial={ser}, got={got}, |diff|={}, scale={scale}",
                (got - ser).abs()
            );
        }

        Ok(())
    }

    // Tail-count convention: ceil on the exact rational (B4 regression)

    #[test]
    fn historical_tail_count_is_exact_at_rational_boundaries() -> TestResult {
        // (1 - 0.90) * 1000 is exactly 100 in the rationals; the float
        // product is 99.99999999999999 and a plain floor() dropped a tail
        // scenario, overstating VaR by one quantile step.
        let n_scenarios = 1000;
        let pnls: Vec<f64> = (0..n_scenarios).map(|s| s as f64 - 500.0).collect();
        let ids = [String::from("A")];
        let config = DecompositionConfig::historical(0.90);

        let result =
            HistoricalPositionDecomposer.decompose_from_pnls(&pnls, &ids, n_scenarios, &config)?;

        // Tail = worst 100 scenarios [-500, -401]; VaR = boundary index 99.
        assert!(
            (result.portfolio_var - (-401.0)).abs() < 1e-12,
            "portfolio_var = {}, expected -401 (100-scenario tail)",
            result.portfolio_var
        );
        assert!(
            (result.portfolio_es - (-450.5)).abs() < 1e-9,
            "portfolio_es = {}, expected -450.5",
            result.portfolio_es
        );
        Ok(())
    }

    #[test]
    fn historical_requires_two_tail_scenarios() {
        // 100 scenarios at 99%: the exact tail is a single observation. A
        // one-scenario tail cannot support a VaR/ES split; it must be
        // rejected like the simulation engine's tail >= 2 guard.
        let n_scenarios = 100;
        let pnls: Vec<f64> = (0..n_scenarios).map(|s| s as f64 - 50.0).collect();
        let ids = [String::from("A")];
        let config = DecompositionConfig::historical(0.99);

        let result =
            HistoricalPositionDecomposer.decompose_from_pnls(&pnls, &ids, n_scenarios, &config);
        assert!(
            result.is_err(),
            "single-observation tail must be rejected, got {result:?}"
        );
    }

    #[test]
    fn stress_attribution_tail_count_is_exact_at_rational_boundaries() -> TestResult {
        // Same exact-rational boundary as the historical engine: the tail
        // at 90% of 1000 scenarios is exactly 100 scenarios.
        let n_scenarios = 1000;
        let pnls: Vec<f64> = (0..n_scenarios).map(|s| s as f64 - 500.0).collect();
        let ids = [String::from("A")];

        let attr = build_stress_attribution(&ids, &pnls, n_scenarios, 0.90)?;
        assert_eq!(attr.n_tail_scenarios, 100);
        Ok(())
    }

    // B5 regression: negative w' Sigma w must error, not clamp to VaR = -0.

    #[test]
    fn parametric_rejects_negative_portfolio_variance() {
        // Indefinite covariance producing w' Sigma w = -2.4e-11 < 0. The
        // factor-level ParametricDecomposer rejects this via
        // validated_variance; the position-level twin must match instead of
        // clamping to zero risk with only a tracing warning.
        let weights = [1.0, -2.5e-5];
        let covariance = [1e-12, 1e-6, 1e-6, 0.04];
        let ids = [String::from("A"), String::from("B")];
        let config = DecompositionConfig::parametric_95();

        let result =
            ParametricPositionDecomposer.decompose_positions(&weights, &covariance, &ids, &config);
        assert!(
            result.is_err(),
            "negative portfolio variance must error, got {result:?}"
        );
    }

    // Gain-tail clamp: totals and components must clamp together.

    #[test]
    fn historical_gain_tail_zeroes_components_and_totals() -> TestResult {
        // Every scenario is a gain: the tail quantile P&L is positive, so
        // both totals clamp to zero. Components must clamp with them (Euler
        // consistency) and the degenerate VaR/ES proration ratio falls back
        // to 0 (no proration when ES ~ 0), matching the simulation engine.
        let n_scenarios = 200;
        let pnls: Vec<f64> = (0..n_scenarios).map(|s| 1.0 + s as f64).collect();
        let ids = [String::from("A")];
        let config = DecompositionConfig::historical(0.95);

        let result =
            HistoricalPositionDecomposer.decompose_from_pnls(&pnls, &ids, n_scenarios, &config)?;

        assert_eq!(result.portfolio_var, 0.0);
        assert_eq!(result.portfolio_es, 0.0);
        assert_eq!(result.var_contributions[0].component_var, 0.0);
        assert_eq!(result.es_contributions[0].component_es, 0.0);
        Ok(())
    }
}
