use std::collections::BTreeMap;

use finstack_quant_core::dates::Date;
use finstack_quant_core::types::IssuerId;

use super::config::{BucketWeighting, PanelSpace};
use super::inputs::CreditCalibrationInputs;
use crate::factor::credit::hierarchy::{IssuerBetaMode, IssuerBetaOverride, IssuerBetaPolicy};
use crate::factor::credit::peel::dts_spread_duration;
use crate::factor::credit::units::decimal_to_bp;

/// Step 1: classify an issuer as `IssuerBeta` or `BucketOnly`.
///
/// Under `Dynamic { min_history, .. }` the gate counts the observations the
/// regression will actually use in the configured panel space: raw `Some`
/// levels for [`PanelSpace::Levels`], consecutive `Some` pairs (usable return
/// observations) for [`PanelSpace::Returns`]. Counting raw levels under
/// `Returns` would overstate the usable history on gappy panels.
pub(super) fn classify_mode(
    policy: &IssuerBetaPolicy,
    issuer: &IssuerId,
    spreads: &BTreeMap<IssuerId, Vec<Option<f64>>>,
    space: &PanelSpace,
) -> IssuerBetaMode {
    match policy {
        IssuerBetaPolicy::GloballyOff => IssuerBetaMode::BucketOnly,
        IssuerBetaPolicy::Dynamic {
            min_history,
            overrides,
        } => match overrides.get(issuer) {
            Some(IssuerBetaOverride::ForceIssuerBeta) => IssuerBetaMode::IssuerBeta,
            Some(IssuerBetaOverride::ForceBucketOnly) => IssuerBetaMode::BucketOnly,
            Some(IssuerBetaOverride::Auto) | None => {
                let count = spreads
                    .get(issuer)
                    .map(|s| match space {
                        PanelSpace::Levels => s.iter().filter(|v| v.is_some()).count(),
                        PanelSpace::Returns => s
                            .windows(2)
                            .filter(|w| w[0].is_some() && w[1].is_some())
                            .count(),
                    })
                    .unwrap_or(0);
                if count >= *min_history {
                    IssuerBetaMode::IssuerBeta
                } else {
                    IssuerBetaMode::BucketOnly
                }
            }
        },
    }
}

/// Working panel after step 2 (returns or levels).
pub(super) struct WorkingPanel {
    /// Generic factor series in the chosen space, length = dates.len() - 1 (Returns)
    /// or dates.len() (Levels).
    pub(super) generic: Vec<f64>,
    /// Per-issuer aligned values (`None` for missing observations / missing pair).
    pub(super) issuers: BTreeMap<IssuerId, Vec<Option<f64>>>,
}

/// First-difference a sparse series: `d[t] = s[t+1] - s[t]` where both
/// observations exist, `None` otherwise. Length is `len - 1`.
pub(crate) fn diff_sparse(series: &[Option<f64>]) -> Vec<Option<f64>> {
    series
        .windows(2)
        .map(|w| match (w[0], w[1]) {
            (Some(a), Some(b)) => Some(b - a),
            _ => None,
        })
        .collect()
}

/// Convert every caller-supplied decimal spread and generic level to bp.
///
/// # Arguments
///
/// * `inputs` - Validated calibration inputs whose spread panel, generic
///   series, `as_of_spreads`, and annualized idiosyncratic-vol overrides are
///   still in decimal spread units. Mutated in place so every subsequent
///   peel/vol step sees basis points (volatility per square-root year).
pub(super) fn convert_inputs_to_bp(inputs: &mut CreditCalibrationInputs) {
    for series in inputs.history_panel.spreads.values_mut() {
        for spread in series.iter_mut().flatten() {
            *spread = decimal_to_bp(*spread);
        }
    }
    for value in &mut inputs.generic_factor.values {
        *value = decimal_to_bp(*value);
    }
    for spread in inputs.as_of_spreads.values_mut() {
        *spread = decimal_to_bp(*spread);
    }
    for volatility in inputs.idiosyncratic_overrides.values_mut() {
        *volatility = decimal_to_bp(*volatility);
    }
}

/// Per-issuer, per-date bucket weights for the historical peel, aligned to
/// the working panel produced by [`build_working_panel`].
///
/// [`BucketWeighting::Equal`] yields `1.0` at every working-panel index.
/// [`BucketWeighting::Dts`] yields `SD_i (years) × S_i(t) (bp)` where `S_i(t)`
/// is the **contemporaneous** panel spread: under [`PanelSpace::Levels`] the
/// spread at date `t`, under [`PanelSpace::Returns`] the begin-of-period
/// spread of the move `t → t+1`. Using the as-of cross-section for every
/// historical date would leak end-of-window information into factor history
/// construction (names that widened into as-of would be overweighted
/// throughout the window), biasing betas and Σ for any backtest.
///
/// # Arguments
///
/// * `weighting` - Equal (`1.0` each) or DTS (`SD_years × spread_bp`).
/// * `space` - Working-panel space; controls the output length
///   (`dates.len()` for Levels, `dates.len() − 1` for Returns) and which
///   date's spread weights each observation.
/// * `spreads_bp` - Complete history panel in **bp** (post
///   [`convert_inputs_to_bp`]); completeness is enforced by input validation.
/// * `spread_durations` - Duration in years, required and `> 0` when `Dts`.
///
/// # Errors
///
/// Returns a diagnostic string when DTS is selected and a duration is
/// missing, non-finite, or `<= 0`, a panel observation is missing, or
/// `SD × spread_bp` is not `> 0` at any date.
pub(super) fn issuer_bucket_weight_series(
    weighting: BucketWeighting,
    space: &PanelSpace,
    spreads_bp: &BTreeMap<IssuerId, Vec<Option<f64>>>,
    spread_durations: &BTreeMap<IssuerId, f64>,
) -> Result<BTreeMap<IssuerId, Vec<f64>>, String> {
    let mut out = BTreeMap::new();
    for (issuer, series) in spreads_bp {
        let len = match space {
            PanelSpace::Levels => series.len(),
            PanelSpace::Returns => series.len().saturating_sub(1),
        };
        let weights = match weighting {
            BucketWeighting::Equal => vec![1.0; len],
            BucketWeighting::Dts => {
                let sd = dts_spread_duration(issuer, spread_durations)?;
                let mut weights = Vec::with_capacity(len);
                for t in 0..len {
                    let spread = series.get(t).copied().flatten().ok_or_else(|| {
                        format!(
                            "dts weighting requires a spread (bp) for issuer {:?} at panel \
                             index {t}",
                            issuer.as_str()
                        )
                    })?;
                    let dts = sd * spread;
                    if !dts.is_finite() || dts <= 0.0 {
                        return Err(format!(
                            "dts for issuer {:?} at panel index {t} must be > 0 \
                             (spread_duration {sd} × spread_bp {spread} = {dts})",
                            issuer.as_str()
                        ));
                    }
                    weights.push(dts);
                }
                weights
            }
        };
        out.insert(issuer.clone(), weights);
    }
    Ok(out)
}

pub(super) fn build_working_panel(
    space: &PanelSpace,
    dates: &[Date],
    spreads: &BTreeMap<IssuerId, Vec<Option<f64>>>,
    generic: &[f64],
) -> WorkingPanel {
    match space {
        PanelSpace::Levels => WorkingPanel {
            generic: generic.to_vec(),
            issuers: spreads.clone(),
        },
        PanelSpace::Returns => {
            let n = dates.len();
            let mut g = Vec::with_capacity(n.saturating_sub(1));
            for t in 1..n {
                g.push(generic[t] - generic[t - 1]);
            }
            let mut issuers: BTreeMap<IssuerId, Vec<Option<f64>>> = BTreeMap::new();
            for (issuer, series) in spreads {
                issuers.insert(issuer.clone(), diff_sparse(series));
            }
            WorkingPanel {
                generic: g,
                issuers,
            }
        }
    }
}
