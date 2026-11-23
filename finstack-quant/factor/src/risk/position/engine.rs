//! Position risk decomposition implementation.

use serde::{Deserialize, Serialize};
use tracing::warn;

/// Method used for position-level VaR/ES decomposition.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum DecompositionMethod {
    /// Covariance-based using normal distribution assumption.
    ///
    /// O(n^2) Euler allocation in positions, after an O(n^3) pivoted-Cholesky
    /// PSD check of the covariance. Exact Euler property.
    /// Requires a position-level return covariance matrix.
    Parametric,

    /// Full historical simulation with per-position P&L attribution.
    ///
    /// Slow (O(n * scenarios)). Approximate Euler property.
    /// Handles non-normality and non-linear positions.
    Historical,
}

// Per-position result structs

/// Risk decomposition result for a single portfolio position.
///
/// All monetary fields are in the same units as the portfolio VaR
/// (typically the portfolio's base currency).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PositionEsContribution {
    /// Position identifier.
    pub position_id: String,

    /// Component ES: position's contribution to portfolio Expected Shortfall.
    ///
    /// Parametric: analytical formula using truncated normal moments.
    /// ```text
    /// CES_i = -w_i * (Sigma * w)_i / sigma_p * phi(z_alpha) / (1 - alpha)
    /// ```
    ///
    /// Historical: probability-weighted position P&L over the exact loss-tail
    /// mass. Boundary ties share their tail weight equally.
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
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
    /// units as [`Self::portfolio_var`]).
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::InputError::NotFound`] when the
    /// decomposition holds no contribution for `position_id`.
    pub fn component_var(&self, position_id: &str) -> finstack_quant_core::Result<f64> {
        self.var_contributions
            .iter()
            .find(|c| c.position_id == position_id)
            .map(|c| c.component_var)
            .ok_or_else(|| {
                finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
                    id: format!("position '{position_id}' in decomposition"),
                })
            })
    }
}

// Shared math helpers

use super::super::{MinTail, PreparedHistoricalScenarios, VARIANCE_TOLERANCE, tail_multipliers};

// Validation helpers

fn validate_decomposition_inputs(
    weights: &[f64],
    covariance: &[f64],
    position_ids: &[String],
    confidence: f64,
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

    crate::config::validate_confidence(confidence)?;

    if weights.iter().any(|v| !v.is_finite()) {
        return Err(finstack_quant_core::Error::Validation(
            "weight entries must be finite".to_string(),
        ));
    }

    // Finiteness, symmetry and positive semi-definiteness via the
    // pivoted Cholesky supplied by core. The factor-level engine instead
    // consumes an immutable, already validated `FactorCovarianceMatrix`. A
    // strict positive-definite factorization
    // would reject rank-deficient PSD covariance — perfectly collinear
    // positions, factor structure `B Σ_f Bᵀ + D` with singular `Σ_f`, or
    // sample covariance with fewer observations than positions — all of
    // which are valid risk inputs: Euler allocation only requires
    // `σ_p = √(wᵀΣw) ≥ 0`.
    if n > 0 {
        crate::covariance::validate_covariance(covariance, n)?;
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
///   var_excl_k      = -z_α · sqrt(max(variance_excl_k, 0))
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
    z_alpha: f64,
    n: usize,
) -> Vec<f64> {
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

/// Decompose portfolio VaR and ES into position contributions via parametric
/// Euler allocation.
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
/// # Arguments
///
/// * `position_ids` - Position identifiers aligned with `weights`.
/// * `weights` - Position weights or exposures (length `n_positions`).
/// * `covariance` - Position-return covariance matrix (`n x n`, row-major,
///   symmetric positive semidefinite).
/// * `confidence` - Tail confidence as a decimal probability strictly inside
///   `(0.5, 1)`. `None` defaults to `0.95`.
/// * `compute_incremental` - Whether to compute leave-one-out incremental VaR
///   (one full repricing per position).
///
/// # Errors
///
/// Returns a validation error if dimensions are inconsistent, the covariance
/// matrix is invalid, or the confidence level is out of bounds.
///
/// # References
///
/// - Litterman (1996): Hot Spots and Hedges. `docs/REFERENCES.md#litterman-1996-hotspots`
/// - Tasche (2008): Capital allocation with Euler's method. `docs/REFERENCES.md#tasche-2008-capital-allocation`
pub fn parametric_var_decomposition(
    position_ids: &[String],
    weights: &[f64],
    covariance: &[f64],
    confidence: Option<f64>,
    compute_incremental: bool,
) -> finstack_quant_core::Result<PositionRiskDecomposition> {
    let confidence = confidence.unwrap_or(0.95);
    validate_decomposition_inputs(weights, covariance, position_ids, confidence)?;

    let n = weights.len();

    if n == 0 {
        return Ok(PositionRiskDecomposition {
            portfolio_var: 0.0,
            portfolio_es: 0.0,
            confidence,
            method: DecompositionMethod::Parametric,
            var_contributions: Vec::new(),
            es_contributions: Vec::new(),
            n_positions: 0,
            euler_residual: Some(0.0),
        });
    }

    let (z_alpha, es_multiplier) = tail_multipliers(confidence)?;

    let sigma_w = super::super::matvec_square(covariance, weights);

    // Portfolio variance = w' * Sigma * w. Reject non-finite or materially negative
    // values (clamping only the numerical rounding band [-tolerance, 0)).
    let mut raw_variance = 0.0;
    for i in 0..n {
        raw_variance += weights[i] * sigma_w[i];
    }
    let variance = super::super::validated_variance(raw_variance)?;
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
    if compute_incremental && n > 1 {
        let incremental = compute_incremental_var(
            weights,
            &sigma_w,
            covariance,
            variance,
            portfolio_var,
            z_alpha,
            n,
        );
        for (contribution, ivar) in var_contributions.iter_mut().zip(incremental) {
            contribution.incremental_var = Some(ivar);
        }
    } else if compute_incremental && n == 1 {
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
        confidence,
        method: DecompositionMethod::Parametric,
        var_contributions,
        es_contributions,
        n_positions: n,
        euler_residual,
    })
}

// Historical simulation engine

/// Decompose portfolio VaR and ES from per-position scenario P&Ls via
/// historical simulation.
///
/// Decomposes VaR and ES by attributing portfolio losses to individual
/// positions within the exact empirical tail mass. Position ES contributions
/// use the same probability weights as portfolio ES and sum to its total.
///
/// # Algorithm
///
/// 1. Compute portfolio P&L for each scenario: PnL_p(s) = sum_i PnL_i(s)
/// 2. Sort scenarios by portfolio P&L (ascending = worst first)
/// 3. Assign exactly `(1-confidence) * n_scenarios` observations of tail mass,
///    sharing fractional boundary mass equally across tied portfolio P&Ls.
/// 4. Component ES: weighted mean of each position's signed P&L in that tail.
/// 5. Component VaR: CVaR_i = CES_i * (VaR_p / ES_p)  (Tasche scaling)
///
/// # Arguments
///
/// * `position_ids` - Position identifiers, length `n_positions`.
/// * `position_pnls` - Scenario-major flat P&L buffer:
///   `position_pnls[s * n_positions + i]` is position `i`'s P&L under
///   scenario `s` (losses negative).
/// * `n_scenarios` - Number of scenario rows encoded in `position_pnls`.
/// * `confidence` - Tail confidence as a decimal probability strictly inside
///   `(0.5, 1)`. `None` defaults to `0.95`.
///
/// # Errors
///
/// Returns a validation error if dimensions are inconsistent, too few
/// scenarios resolve the tail, or the confidence level is out of bounds.
///
/// # References
///
/// - Hallerbach (2003): Decomposing portfolio Value-at-Risk. `docs/REFERENCES.md#hallerbach-2003-decomposing-var`
pub fn historical_var_decomposition(
    position_ids: &[String],
    position_pnls: &[f64],
    n_scenarios: usize,
    confidence: Option<f64>,
) -> finstack_quant_core::Result<PositionRiskDecomposition> {
    let prepared = PreparedHistoricalScenarios::prepare(
        position_ids,
        position_pnls,
        n_scenarios,
        confidence,
        MinTail::AtLeastTwo,
    )?;
    Ok(historical_var_decomposition_prepared(&prepared))
}

pub(in crate::risk) fn historical_var_decomposition_prepared(
    prepared: &PreparedHistoricalScenarios<'_>,
) -> PositionRiskDecomposition {
    let position_ids = prepared.position_ids;
    let position_pnls = prepared.position_pnls;
    let n_scenarios = prepared.n_scenarios;
    let confidence = prepared.confidence;
    let n = position_ids.len();

    if n == 0 || n_scenarios == 0 {
        return PositionRiskDecomposition {
            portfolio_var: 0.0,
            portfolio_es: 0.0,
            confidence,
            method: DecompositionMethod::Historical,
            var_contributions: Vec::new(),
            es_contributions: Vec::new(),
            n_positions: n,
            euler_residual: None,
        };
    }

    let n_tail = super::super::tail_scenario_count(confidence, n_scenarios);
    let portfolio_pnls = &prepared.portfolio_pnls;

    // Portfolio VaR: the signed P&L at the tail boundary scenario. The
    // tail spans sorted indices 0..n_tail (ascending P&L), so the VaR
    // threshold is the least-bad scenario of the tail, index n_tail-1.
    //
    // Loss convention (workspace-wide): VaR/ES follow the P&L sign, so
    // losses are negative. Clamp to zero only when the quantile P&L is
    // actually a gain.
    let var_idx = (n_tail - 1).min(n_scenarios - 1);
    let portfolio_var = portfolio_pnls[var_idx].1.min(0.0);

    // ES integrates exactly (1-confidence) of the empirical probability
    // mass. Including an entire fractional boundary observation changes
    // the risk measure when the same empirical sample is duplicated.
    let raw_tail_mass = (1.0 - confidence) * n_scenarios as f64;
    let tail_mass = if (raw_tail_mass - raw_tail_mass.round()).abs()
        < super::super::TAIL_COUNT_SNAP_TOLERANCE
    {
        raw_tail_mass.round()
    } else {
        raw_tail_mass
    };
    let boundary_pnl = portfolio_pnls[var_idx].1;
    let before_boundary = portfolio_pnls.partition_point(|(_, pnl)| *pnl < boundary_pnl);
    let after_boundary = portfolio_pnls.partition_point(|(_, pnl)| *pnl <= boundary_pnl);
    let boundary_weight =
        (tail_mass - before_boundary as f64) / (after_boundary - before_boundary) as f64;

    // All scenarios tied at the boundary receive the same fractional
    // weight, so position allocation cannot depend on their input order.
    let mut raw_portfolio_es = 0.0;
    let mut component_es_vec = vec![0.0; n];
    for (rank, &(s, pnl)) in portfolio_pnls[..after_boundary].iter().enumerate() {
        let weight = if rank < before_boundary {
            1.0
        } else {
            boundary_weight
        };
        raw_portfolio_es += weight * pnl;
        let row_start = s * n;
        for i in 0..n {
            component_es_vec[i] += weight * position_pnls[row_start + i];
        }
    }
    raw_portfolio_es /= tail_mass;
    let portfolio_es = raw_portfolio_es.min(0.0);
    for ces in component_es_vec.iter_mut() {
        *ces /= tail_mass;
    }

    // Gain-clamp Euler consistency: when the tail mean is a gain the
    // total ES clamps to zero above, so the components must be zeroed in
    // the same branch or they no longer sum to the total.
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
    PositionRiskDecomposition {
        portfolio_var,
        portfolio_es,
        confidence,
        method: DecompositionMethod::Historical,
        var_contributions,
        es_contributions,
        n_positions: n,
        euler_residual: None,
    }
}

/// Flatten per-position scenario P&Ls into a scenario-major buffer.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] when the number of rows
/// does not equal `n_positions`, rows have inconsistent scenario counts, or the
/// flattened buffer size overflows `usize`.
///
/// # Arguments
///
/// * `position_pnls` - Position-major P&L matrix with one row per position.
/// * `n_positions` - Expected number of position rows.
pub fn flatten_position_pnls(
    position_pnls: &[Vec<f64>],
    n_positions: usize,
) -> finstack_quant_core::Result<(Vec<f64>, usize)> {
    if position_pnls.len() != n_positions {
        return Err(finstack_quant_core::Error::Validation(format!(
            "position_pnls must have {n_positions} rows, got {}",
            position_pnls.len()
        )));
    }
    if n_positions == 0 {
        return Ok((Vec::new(), 0));
    }
    let n_scenarios = position_pnls[0].len();
    for (index, row) in position_pnls.iter().enumerate() {
        if row.len() != n_scenarios {
            return Err(finstack_quant_core::Error::Validation(format!(
                "position_pnls row {index} has {} scenarios, expected {n_scenarios}",
                row.len()
            )));
        }
    }
    let capacity = n_scenarios.checked_mul(n_positions).ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "flattened P&L buffer size overflows usize for n_scenarios ({n_scenarios}) * \
             n_positions ({n_positions})"
        ))
    })?;
    let mut flat = Vec::with_capacity(capacity);
    for scenario in 0..n_scenarios {
        for row in position_pnls {
            flat.push(row[scenario]);
        }
    }
    Ok((flat, n_scenarios))
}
